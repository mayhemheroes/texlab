use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use log::{error, info, warn};
use lsp_server::{Connection, Message, RequestId};
use lsp_types::{notification::*, request::*, *};
use salsa::ParallelDatabase;
use serde::Serialize;
use threadpool::ThreadPool;

use crate::{
    client::send_request,
    db::*,
    dispatch::{NotificationDispatcher, RequestDispatcher},
    distro::Distribution,
    features::{
        find_all_references, find_document_highlights, find_document_links, find_document_symbols,
        find_foldings, find_hover, find_workspace_symbols, format_source_code, goto_definition,
        prepare_rename_all, rename_all, BuildEngine, BuildParams, BuildResult, BuildStatus,
        FeatureRequest, ForwardSearchResult, ForwardSearchStatus,
    },
    req_queue::{IncomingData, ReqQueue},
    DocumentLanguage, LineIndex, LineIndexExt, Options,
};

#[derive(Debug)]
enum InternalMessage {
    SetDistro(Distribution),
    SetOptions(Options),
}

#[derive(Clone)]
struct SharedState {
    connection: Arc<Connection>,
    internal_tx: Sender<InternalMessage>,
    req_queue: Arc<Mutex<ReqQueue>>,
    // static_debouncer: Arc<DiagnosticsDebouncer>,
    // chktex_debouncer: Arc<DiagnosticsDebouncer>,
    pool: Arc<Mutex<ThreadPool>>,
    build_engine: Arc<BuildEngine>,
}

impl SharedState {
    pub fn spawn(&self, job: impl FnOnce(Self) + Send + 'static) {
        let state = self.clone();
        self.pool.lock().unwrap().execute(move || job(state));
    }

    pub fn register_file_watching(&self, db: &salsa::Snapshot<RootDatabase>) {
        if db.has_file_watching_support() {
            let options = DidChangeWatchedFilesRegistrationOptions {
                watchers: vec![FileSystemWatcher {
                    glob_pattern: "**/*.{aux,log}".into(),
                    kind: Some(WatchKind::Create | WatchKind::Change | WatchKind::Delete),
                }],
            };

            let reg = Registration {
                id: "build-watch".to_string(),
                method: DidChangeWatchedFiles::METHOD.to_string(),
                register_options: Some(serde_json::to_value(options).unwrap()),
            };

            let params = RegistrationParams {
                registrations: vec![reg],
            };

            if let Err(why) =
                send_request::<RegisterCapability>(&self.req_queue, &self.connection.sender, params)
            {
                error!(
                    "Failed to register \"{}\" notification: {}",
                    DidChangeWatchedFiles::METHOD,
                    why
                );
            }
        }
    }

    pub fn register_config_capability(&self, db: &salsa::Snapshot<RootDatabase>) {
        if db.has_push_configuration_support() {
            let reg = Registration {
                id: "push-config".to_string(),
                method: DidChangeConfiguration::METHOD.to_string(),
                register_options: None,
            };

            let params = RegistrationParams {
                registrations: vec![reg],
            };

            if let Err(why) =
                send_request::<RegisterCapability>(&self.req_queue, &self.connection.sender, params)
            {
                error!(
                    "Failed to register \"{}\" notification: {}",
                    DidChangeConfiguration::METHOD,
                    why
                );
            }
        }
    }

    pub fn pull_config(&self, db: &salsa::Snapshot<RootDatabase>) {
        if !db.has_pull_configuration_support() {
            return;
        }

        let params = ConfigurationParams {
            items: vec![ConfigurationItem {
                section: Some("texlab".to_string()),
                scope_uri: None,
            }],
        };

        match send_request::<WorkspaceConfiguration>(
            &self.req_queue,
            &self.connection.sender,
            params,
        ) {
            Ok(mut json) => {
                let value = json.pop().expect("invalid configuration request");
                let options = match serde_json::from_value(value) {
                    Ok(new_options) => new_options,
                    Err(why) => {
                        warn!("Invalid configuration section \"texlab\": {}", why);
                        Options::default()
                    }
                };

                self.internal_tx
                    .send(InternalMessage::SetOptions(options))
                    .unwrap();
            }
            Err(why) => {
                error!("Retrieving configuration failed: {}", why);
            }
        };
    }

    pub fn register_incoming_request(&self, id: RequestId) {
        self.req_queue
            .lock()
            .unwrap()
            .incoming
            .register(id, IncomingData);
    }
}

pub struct Server {
    state: SharedState,
    internal_rx: Receiver<InternalMessage>,
    db: RootDatabase,
    load_resolver: bool,
}

impl Server {
    pub fn with_connection(
        connection: Connection,
        current_dir: PathBuf,
        load_resolver: bool,
    ) -> Result<Self> {
        let req_queue = Arc::default();
        let mut db = RootDatabase::default();
        db.set_current_directory(Arc::new(current_dir));

        // let diag_manager = Arc::new(Mutex::new(DiagnosticsManager::default()));

        // let static_debouncer = Arc::new(create_static_debouncer(
        //     Arc::clone(&diag_manager),
        //     &connection,
        // ));

        // let chktex_debouncer = Arc::new(create_chktex_debouncer(diag_manager, &connection));

        let (internal_tx, internal_rx) = crossbeam_channel::unbounded();

        let state = SharedState {
            connection: Arc::new(connection),
            internal_tx,
            req_queue,
            // static_debouncer,
            // chktex_debouncer,
            pool: Arc::new(Mutex::new(threadpool::Builder::new().build())),
            build_engine: Arc::default(),
        };

        Ok(Self {
            state,
            internal_rx,
            db,
            load_resolver,
        })
    }

    fn capabilities(&self) -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Options(
                TextDocumentSyncOptions {
                    open_close: Some(true),
                    change: Some(TextDocumentSyncKind::INCREMENTAL),
                    will_save: None,
                    will_save_wait_until: None,
                    save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                        include_text: Some(false),
                    })),
                },
            )),
            document_link_provider: Some(DocumentLinkOptions {
                resolve_provider: Some(false),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            }),
            folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            #[cfg(feature = "completion")]
            completion_provider: Some(CompletionOptions {
                resolve_provider: Some(true),
                trigger_characters: Some(vec![
                    "\\".into(),
                    "{".into(),
                    "}".into(),
                    "@".into(),
                    "/".into(),
                    " ".into(),
                ]),
                ..CompletionOptions::default()
            }),
            document_symbol_provider: Some(OneOf::Left(true)),
            workspace_symbol_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Right(RenameOptions {
                prepare_provider: Some(true),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            })),
            document_highlight_provider: Some(OneOf::Left(true)),
            document_formatting_provider: Some(OneOf::Left(true)),
            ..ServerCapabilities::default()
        }
    }

    fn initialize(&mut self) -> Result<()> {
        let (id, params) = self.state.connection.initialize_start()?;
        let params: InitializeParams = serde_json::from_value(params)?;

        self.db
            .set_client_capabilities(Arc::new(params.capabilities));
        self.db.set_client_info(params.client_info.map(Arc::new));

        let result = InitializeResult {
            capabilities: self.capabilities(),
            server_info: Some(ServerInfo {
                name: "TexLab".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        };
        self.state
            .connection
            .initialize_finish(id, serde_json::to_value(result)?)?;

        if self.load_resolver {
            self.state.spawn(move |server| {
                let distro = Distribution::detect();
                info!("Detected distribution: {}", distro.kind);

                server
                    .internal_tx
                    .send(InternalMessage::SetDistro(distro))
                    .unwrap();
            });
        }

        // self.register_diagnostics_handler();

        let db = self.db.snapshot();
        self.state.spawn(move |state| {
            state.register_config_capability(&db);
            state.register_file_watching(&db);
            state.pull_config(&db);
        });

        Ok(())
    }

    // fn register_diagnostics_handler(&mut self) {
    //     let (event_sender, event_receiver) = crossbeam_channel::unbounded();
    //     let diag_sender = self.state.static_debouncer.sender.clone();
    //     std::thread::spawn(move || {
    //         for event in event_receiver {
    //             match event {
    //                 WorkspaceEvent::Changed(workspace, document) => {
    //                     let message = DiagnosticsMessage::Analyze {
    //                         workspace,
    //                         document,
    //                     };

    //                     diag_sender.send(message).unwrap();
    //                 }
    //             };
    //         }
    //     });

    //     self.workspace.listeners.push_back(event_sender);
    // }

    fn cancel(&self, params: CancelParams) -> Result<()> {
        let id = match params.id {
            NumberOrString::Number(id) => RequestId::from(id),
            NumberOrString::String(id) => RequestId::from(id),
        };

        let mut req_queue = self.state.req_queue.lock().unwrap();
        req_queue.incoming.complete(id);

        Ok(())
    }

    fn did_change_watched_files(&mut self, params: DidChangeWatchedFilesParams) -> Result<()> {
        for change in params.changes {
            if let Ok(path) = change.uri.to_file_path() {
                let document = self.db.intern_document(DocumentData::from(change.uri));
                match change.typ {
                    FileChangeType::CREATED | FileChangeType::CHANGED => {
                        let _ = self.db.insert_hidden_document(&path);
                        self.db
                            .set_visibility(document, DocumentVisibility::Visible);
                    }
                    FileChangeType::DELETED => {
                        let mut all_documents = self.db.all_documents();
                        all_documents.remove(&document);
                        self.db.set_all_documents(all_documents);
                        self.db.set_source_code(document, Arc::new(String::new()));
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn did_change_configuration(&mut self, params: DidChangeConfigurationParams) -> Result<()> {
        if self.db.has_pull_configuration_support() {
            let db = self.db.snapshot();
            self.state.spawn(move |state| {
                state.pull_config(&db);
            });
        } else {
            match serde_json::from_value(params.settings) {
                Ok(options) => {
                    self.db.set_client_options(Arc::new(options));
                }
                Err(why) => {
                    error!("Invalid configuration: {}", why);
                }
            };
        }

        Ok(())
    }

    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<()> {
        let language_id = &params.text_document.language_id;
        let language =
            DocumentLanguage::by_language_id(language_id).unwrap_or(DocumentLanguage::Latex);

        let document_data = DocumentData::from(params.text_document.uri);
        let document = self.db.intern_document(document_data);
        let source_code = Arc::new(params.text_document.text);
        self.db.upsert_document(document, source_code, language);
        self.db
            .set_visibility(document, DocumentVisibility::Visible);

        // if self.workspace.environment.options.chktex.on_open_and_save {
        //     self.state
        //         .chktex_debouncer
        //         .sender
        //         .send(DiagnosticsMessage::Analyze {
        //             workspace: self.workspace.clone(),
        //             document,
        //         })?;
        // }

        Ok(())
    }

    fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Result<()> {
        let document_data = DocumentData::from(params.text_document.uri);
        let document = self.db.intern_document(document_data);

        if self.db.all_documents().contains(&document) {
            let old_text = self.db.source_code(document);
            let mut new_text = old_text.to_string();
            apply_document_edit(&mut new_text, params.content_changes);
            let new_text = Arc::new(new_text);
            self.db.set_source_code(document, Arc::clone(&new_text));

            // let language = old_document.data.language();
            // let new_document =
            //     self.workspace
            //         .open(Arc::clone(&uri), Arc::new(source_code), language)?;
            // self.workspace
            //     .viewport
            //     .insert(Arc::clone(&new_document.uri));

            self.state.build_engine.positions_by_uri.insert(
                document.lookup(&self.db).uri,
                Position::new(
                    old_text
                        .lines()
                        .zip(new_text.lines())
                        .position(|(a, b)| a != b)
                        .unwrap_or_default() as u32,
                    0,
                ),
            );

            // if self.workspace.environment.options.chktex.on_edit {
            //     self.state
            //         .chktex_debouncer
            //         .sender
            //         .send(DiagnosticsMessage::Analyze {
            //             workspace: self.workspace.clone(),
            //             document: new_document,
            //         })?;
            // };
        } else {
            let uri = document.lookup(&self.db).uri;
            if uri.scheme() == "file" {
                if let Ok(path) = document.lookup(&self.db).uri.to_file_path() {
                    let _ = self.db.insert_hidden_document(&path);
                }
            }
        }

        Ok(())
    }

    fn did_save(&self, params: DidSaveTextDocumentParams) -> Result<()> {
        let document_data = DocumentData::from(params.text_document.uri);
        let document = self.db.intern_document(document_data);

        if self.db.all_documents().contains(&document) && self.db.client_options().build.on_save {
            let db = self.db.snapshot();

            self.state.spawn(move |state| {
                let request = FeatureRequest {
                    params: BuildParams {
                        text_document: TextDocumentIdentifier::new(
                            db.lookup_intern_document(document).uri.as_ref().clone(),
                        ),
                    },
                    document,
                    db: &db,
                };

                state
                    .build_engine
                    .build(request, &state.req_queue, &state.connection.sender)
                    .unwrap_or_else(|why| {
                        error!("Build failed: {}", why);
                        BuildResult {
                            status: BuildStatus::FAILURE,
                        }
                    });
            });
        }

        // if let Some(document) = self
        //     .workspace
        //     .documents_by_uri
        //     .get(&uri)
        //     .filter(|_| self.workspace.environment.options.chktex.on_open_and_save)
        //     .cloned()
        // {
        //     self.state
        //         .chktex_debouncer
        //         .sender
        //         .send(DiagnosticsMessage::Analyze {
        //             workspace: self.workspace.clone(),
        //             document,
        //         })?;
        // };
        Ok(())
    }

    fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Result<()> {
        let document = self
            .db
            .intern_document(DocumentData::from(params.text_document.uri));

        self.db.set_visibility(document, DocumentVisibility::Hidden);
        Ok(())
    }

    fn handle_feature_request<P, R, H>(
        &self,
        id: RequestId,
        params: P,
        document: Document,
        handler: H,
    ) -> Result<()>
    where
        P: Send + 'static,
        R: Serialize,
        H: FnOnce(FeatureRequest<P>) -> R + Send + 'static,
    {
        let db = self.db.snapshot();
        self.state.spawn(move |state| {
            let request = FeatureRequest {
                params,
                db: &db,
                document,
            };

            let result = handler(request);
            state
                .connection
                .sender
                .send(lsp_server::Response::new_ok(id, result).into())
                .unwrap();
        });

        Ok(())
    }

    fn document_link(&self, id: RequestId, params: DocumentLinkParams) -> Result<()> {
        let document = self
            .db
            .intern_document(DocumentData::from(params.text_document.uri.clone()));
        self.handle_feature_request(id, params, document, find_document_links)?;
        Ok(())
    }

    fn document_symbols(&self, id: RequestId, params: DocumentSymbolParams) -> Result<()> {
        let document = self
            .db
            .intern_document(DocumentData::from(params.text_document.uri.clone()));
        self.handle_feature_request(id, params, document, find_document_symbols)?;
        Ok(())
    }

    fn workspace_symbols(&self, id: RequestId, params: WorkspaceSymbolParams) -> Result<()> {
        let db = self.db.snapshot();
        self.state.spawn(move |state| {
            let result = find_workspace_symbols(&db, &params);
            state
                .connection
                .sender
                .send(lsp_server::Response::new_ok(id, result).into())
                .unwrap();
        });
        Ok(())
    }

    #[cfg(feature = "completion")]
    fn completion(&self, id: RequestId, params: CompletionParams) -> Result<()> {
        let document = self.db.intern_document(DocumentData::from(
            params.text_document_position.text_document.uri.clone(),
        ));

        self.state.build_engine.positions_by_uri.insert(
            document.lookup(&self.db).uri,
            params.text_document_position.position,
        );

        self.handle_feature_request(id, params, document, crate::features::complete)?;
        Ok(())
    }

    #[cfg(feature = "completion")]
    fn completion_resolve(&self, id: RequestId, mut item: CompletionItem) -> Result<()> {
        match serde_json::from_value(item.data.clone().unwrap()).unwrap() {
            crate::features::CompletionItemData::Package
            | crate::features::CompletionItemData::Class => {
                item.documentation = crate::component_db::COMPONENT_DATABASE
                    .documentation(&item.label)
                    .map(Documentation::MarkupContent);
            }
            #[cfg(feature = "citation")]
            crate::features::CompletionItemData::Citation { uri, key } => {
                let document = self.db.intern_document(DocumentData::from(uri));
                if self.db.all_documents().contains(&document) {
                    if let SyntaxTree::Bibtex(green) = self.db.syntax_tree(document) {
                        let markup = crate::citation::render_citation(
                            &crate::syntax::bibtex::SyntaxNode::new_root(green),
                            &key,
                        );
                        item.documentation = markup.map(Documentation::MarkupContent);
                    }
                }
            }
            _ => {}
        };

        self.state
            .connection
            .sender
            .send(lsp_server::Response::new_ok(id, item).into())
            .unwrap();
        Ok(())
    }

    fn folding_range(&self, id: RequestId, params: FoldingRangeParams) -> Result<()> {
        let document = self
            .db
            .intern_document(DocumentData::from(params.text_document.uri.clone()));
        self.handle_feature_request(id, params, document, find_foldings)?;
        Ok(())
    }

    fn references(&self, id: RequestId, params: ReferenceParams) -> Result<()> {
        let document = self.db.intern_document(DocumentData::from(
            params.text_document_position.text_document.uri.clone(),
        ));
        self.handle_feature_request(id, params, document, find_all_references)?;
        Ok(())
    }

    fn hover(&self, id: RequestId, params: HoverParams) -> Result<()> {
        let document = self.db.intern_document(DocumentData::from(
            params
                .text_document_position_params
                .text_document
                .uri
                .clone(),
        ));

        self.state.build_engine.positions_by_uri.insert(
            document.lookup(&self.db).uri,
            params.text_document_position_params.position,
        );

        self.handle_feature_request(id, params, document, find_hover)?;
        Ok(())
    }

    fn goto_definition(&self, id: RequestId, params: GotoDefinitionParams) -> Result<()> {
        let document = self.db.intern_document(DocumentData::from(
            params
                .text_document_position_params
                .text_document
                .uri
                .clone(),
        ));

        self.handle_feature_request(id, params, document, goto_definition)?;
        Ok(())
    }

    fn prepare_rename(&self, id: RequestId, params: TextDocumentPositionParams) -> Result<()> {
        let document = self
            .db
            .intern_document(DocumentData::from(params.text_document.uri.clone()));

        self.handle_feature_request(id, params, document, prepare_rename_all)?;
        Ok(())
    }

    fn rename(&self, id: RequestId, params: RenameParams) -> Result<()> {
        let document = self.db.intern_document(DocumentData::from(
            params.text_document_position.text_document.uri.clone(),
        ));

        self.handle_feature_request(id, params, document, rename_all)?;
        Ok(())
    }

    fn document_highlight(&self, id: RequestId, params: DocumentHighlightParams) -> Result<()> {
        let document = self.db.intern_document(DocumentData::from(
            params
                .text_document_position_params
                .text_document
                .uri
                .clone(),
        ));

        self.handle_feature_request(id, params, document, find_document_highlights)?;
        Ok(())
    }

    fn formatting(&self, id: RequestId, params: DocumentFormattingParams) -> Result<()> {
        let document = self
            .db
            .intern_document(DocumentData::from(params.text_document.uri.clone()));
        self.handle_feature_request(id, params, document, format_source_code)?;
        Ok(())
    }

    fn semantic_tokens_range(
        &self,
        _id: RequestId,
        _params: SemanticTokensRangeParams,
    ) -> Result<()> {
        Ok(())
    }

    fn build(&self, id: RequestId, params: BuildParams) -> Result<()> {
        let document = self
            .db
            .intern_document(DocumentData::from(params.text_document.uri.clone()));
        let lsp_sender = self.state.connection.sender.clone();
        let req_queue = Arc::clone(&self.state.req_queue);
        let build_engine = Arc::clone(&self.state.build_engine);
        self.handle_feature_request(id, params, document, move |request| {
            build_engine
                .build(request, &req_queue, &lsp_sender)
                .unwrap_or_else(|why| {
                    error!("Build failed: {}", why);
                    BuildResult {
                        status: BuildStatus::FAILURE,
                    }
                })
        })?;
        Ok(())
    }

    fn forward_search(&self, id: RequestId, params: TextDocumentPositionParams) -> Result<()> {
        let document = self
            .db
            .intern_document(DocumentData::from(params.text_document.uri.clone()));

        self.handle_feature_request(id, params, document, |req| {
            crate::features::execute_forward_search(req).unwrap_or(ForwardSearchResult {
                status: ForwardSearchStatus::ERROR,
            })
        })?;
        Ok(())
    }

    fn process_messages(&mut self) -> Result<()> {
        loop {
            crossbeam_channel::select! {
                recv(&self.state.connection.receiver) -> msg => {
                    match msg? {
                        Message::Request(request) => {
                            if self.state.connection.handle_shutdown(&request)? {
                                return Ok(());
                            }

                            self.state.register_incoming_request(request.id.clone());
                            if let Some(response) = RequestDispatcher::new(request)
                                .on::<DocumentLinkRequest, _>(|id, params| self.document_link(id, params))?
                                .on::<FoldingRangeRequest, _>(|id, params| self.folding_range(id, params))?
                                .on::<References, _>(|id, params| self.references(id, params))?
                                .on::<HoverRequest, _>(|id, params| self.hover(id, params))?
                                .on::<DocumentSymbolRequest, _>(|id, params| {
                                    self.document_symbols(id, params)
                                })?
                                .on::<WorkspaceSymbol, _>(|id, params| self.workspace_symbols(id, params))?
                                .on::<Completion, _>(|id, params| {
                                    #[cfg(feature = "completion")]
                                    self.completion(id, params)?;
                                    Ok(())
                                })?
                                .on::<ResolveCompletionItem, _>(|id, params| {
                                    #[cfg(feature = "completion")]
                                    self.completion_resolve(id, params)?;
                                    Ok(())
                                })?
                                .on::<GotoDefinition, _>(|id, params| self.goto_definition(id, params))?
                                .on::<PrepareRenameRequest, _>(|id, params| {
                                    self.prepare_rename(id, params)
                                })?
                                .on::<Rename, _>(|id, params| self.rename(id, params))?
                                .on::<DocumentHighlightRequest, _>(|id, params| {
                                    self.document_highlight(id, params)
                                })?
                                .on::<Formatting, _>(|id, params| self.formatting(id, params))?
                                .on::<BuildRequest, _>(|id, params| self.build(id, params))?
                                .on::<ForwardSearchRequest, _>(|id, params| {
                                    self.forward_search(id, params)
                                })?
                                .on::<SemanticTokensRangeRequest, _>(|id, params| {
                                    self.semantic_tokens_range(id, params)
                                })?
                                .default()
                            {
                                self.state.connection.sender.send(response.into())?;
                            }
                        }
                        Message::Notification(notification) => {
                            NotificationDispatcher::new(notification)
                                .on::<Cancel, _>(|params| self.cancel(params))?
                                .on::<DidChangeConfiguration, _>(|params| {
                                    self.did_change_configuration(params)
                                })?
                                .on::<DidChangeWatchedFiles, _>(|params| {
                                    self.did_change_watched_files(params)
                                })?
                                .on::<DidOpenTextDocument, _>(|params| self.did_open(params))?
                                .on::<DidChangeTextDocument, _>(|params| self.did_change(params))?
                                .on::<DidSaveTextDocument, _>(|params| self.did_save(params))?
                                .on::<DidCloseTextDocument, _>(|params| self.did_close(params))?
                                .default();
                        }
                        Message::Response(response) => {
                            let mut req_queue = self.state.req_queue.lock().unwrap();
                            if let Some(data) = req_queue.outgoing.complete(response.id) {
                                let result = match response.error {
                                    Some(error) => Err(error),
                                    None => Ok(response.result.unwrap_or_default()),
                                };
                                data.sender.send(result)?;
                            }
                        }
                    };
                },
                recv(&self.internal_rx) -> msg => {
                    match msg? {
                        InternalMessage::SetDistro(distro) => {
                            self.db.set_distro_kind(distro.kind);
                            self.db.set_distro_resolver(Arc::new(distro.resolver));
                        }
                        InternalMessage::SetOptions(options) => {
                            self.db.set_client_options(Arc::new(options));
                        }
                    };
                }
            };
        }
    }

    pub fn run(mut self) -> Result<()> {
        self.initialize()?;
        self.process_messages()?;
        // drop(self.state.static_debouncer);
        // drop(self.state.chktex_debouncer);
        self.state.pool.lock().unwrap().join();
        Ok(())
    }
}

// fn create_static_debouncer(
//     manager: Arc<Mutex<DiagnosticsManager>>,
//     conn: &Connection,
// ) -> DiagnosticsDebouncer {
//     let sender = conn.sender.clone();
//     DiagnosticsDebouncer::launch(move |workspace, document| {
//         let mut manager = manager.lock().unwrap();
//         manager.update_static(&workspace, Arc::clone(&document.uri));
//         if let Err(why) = publish_diagnostics(&sender, &workspace, &manager) {
//             warn!("Failed to publish diagnostics: {}", why);
//         }
//     })
// }

// fn create_chktex_debouncer(
//     manager: Arc<Mutex<DiagnosticsManager>>,
//     conn: &Connection,
// ) -> DiagnosticsDebouncer {
//     let sender = conn.sender.clone();
//     DiagnosticsDebouncer::launch(move |workspace, document| {
//         let mut manager = manager.lock().unwrap();
//         manager.update_chktex(
//             &workspace,
//             Arc::clone(&document.uri),
//             &workspace.environment.options,
//         );
//         if let Err(why) = publish_diagnostics(&sender, &workspace, &manager) {
//             warn!("Failed to publish diagnostics: {}", why);
//         }
//     })
// }

// fn publish_diagnostics(
//     sender: &Sender<lsp_server::Message>,
//     workspace: &Workspace,
//     diag_manager: &DiagnosticsManager,
// ) -> Result<()> {
//     for document in workspace.documents_by_uri.values() {
//         let diagnostics = diag_manager.publish(Arc::clone(&document.uri));
//         send_notification::<PublishDiagnostics>(
//             sender,
//             PublishDiagnosticsParams {
//                 uri: document.uri.as_ref().clone(),
//                 version: None,
//                 diagnostics,
//             },
//         )?;
//     }
//     Ok(())
// }

fn apply_document_edit(old_text: &mut String, changes: Vec<TextDocumentContentChangeEvent>) {
    for change in changes {
        let line_index = LineIndex::new(old_text);
        match change.range {
            Some(range) => {
                let range = std::ops::Range::<usize>::from(line_index.offset_lsp_range(range));
                old_text.replace_range(range, &change.text);
            }
            None => {
                *old_text = change.text;
            }
        };
    }
}

struct BuildRequest;

impl lsp_types::request::Request for BuildRequest {
    type Params = BuildParams;

    type Result = BuildResult;

    const METHOD: &'static str = "textDocument/build";
}

struct ForwardSearchRequest;

impl lsp_types::request::Request for ForwardSearchRequest {
    type Params = TextDocumentPositionParams;

    type Result = ForwardSearchResult;

    const METHOD: &'static str = "textDocument/forwardSearch";
}
