use crate::{distro::Resolver, Uri};

pub fn resolve_distro_file(resolver: &Resolver, stem: &str, extensions: &[&str]) -> Option<Uri> {
    let mut document = resolver.files_by_name.get(stem);
    for extension in extensions {
        document = document.or_else(|| {
            let full_name = format!("{}.{}", stem, extension);
            resolver.files_by_name.get(full_name.as_str())
        });
    }
    document.and_then(|path| Uri::from_file_path(path).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test() {
        let mut resolver = Resolver::default();
        resolver
            .files_by_name
            .insert("foo.tex".into(), "C:/distro/foo.tex".into());
        resolver
            .files_by_name
            .insert("foo.sty".into(), "C:/distro/foo.sty".into());
        resolver
            .files_by_name
            .insert("bar.tex".into(), "C:/distro/bar.tex".into());

        assert_eq!(
            resolve_distro_file(&resolver, "foo", &["tex"]),
            Some(Uri::from_file_path("C:/distro/foo.tex").unwrap())
        );

        assert_eq!(
            resolve_distro_file(&resolver, "foo", &["sty"]),
            Some(Uri::from_file_path("C:/distro/foo.sty").unwrap())
        );

        assert_eq!(resolve_distro_file(&resolver, "foo", &["cls"]), None);
    }

    #[test]
    #[cfg(unix)]
    fn test() {
        let mut resolver = Resolver::default();
        resolver
            .files_by_name
            .insert("foo.tex".into(), "/distro/foo.tex".into());
        resolver
            .files_by_name
            .insert("foo.sty".into(), "/distro/foo.sty".into());
        resolver
            .files_by_name
            .insert("bar.tex".into(), "/distro/bar.tex".into());

        assert_eq!(
            analyze_distro_file(&resolver, "foo", &["tex"]),
            Some(Uri::from_file_path("/distro/foo.tex").unwrap())
        );

        assert_eq!(
            analyze_distro_file(&resolver, "foo", &["sty"]),
            Some(Uri::from_file_path("/distro/foo.sty").unwrap())
        );

        assert_eq!(analyze_distro_file(&resolver, "foo", &["cls"]), None);
    }
}
