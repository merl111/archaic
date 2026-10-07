pub struct Options {
    pub locale: String,
    pub profile: String,
    pub explicit_profile: bool,
    pub link: Option<String>,
    pub theme: Option<crate::appearance::Appearance>,
    pub fixture_mode: bool,
    pub smoke: bool,
    pub temporary: bool,
}

pub fn parse() -> Result<Option<Options>, Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut link = None;
    let mut explicit_profile = false;
    let mut profile = "default".to_owned();
    let mut locale = "en".to_string();
    let mut theme = None;
    let mut fixture_mode = false;
    let mut smoke = false;
    let mut temporary = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--open" => {
                let value = args.next().ok_or("--open requires a Matrix link")?.clone();
                archaic_matrix::links::parse(&value).map_err(|_| "invalid Matrix link")?;
                link = Some(value);
            }
            value if value.starts_with("matrix:") || value.starts_with("https://matrix.to/") => {
                archaic_matrix::links::parse(value).map_err(|_| "invalid Matrix link")?;
                link = Some(value.to_owned());
            }
            "--profile" => {
                explicit_profile = true;
                profile = args.next().ok_or("--profile requires a name")?.clone();
                archaic_matrix::storage::validate_profile_name(&profile)
                    .map_err(|_| "invalid profile name")?;
            }
            "--locale" => locale = args.next().ok_or("--locale requires en or de")?.clone(),
            "--theme" => {
                theme = Some(match args.next().map(String::as_str) {
                    Some("system") => crate::appearance::Appearance::System,
                    Some("light") => crate::appearance::Appearance::Light,
                    Some("dark") => crate::appearance::Appearance::Dark,
                    _ => return Err("--theme requires system, light or dark".into()),
                });
            }
            "--temporary-session" => temporary = true,
            "--fixture" => fixture_mode = true,
            "--smoke-test" => smoke = true,
            "--help" => {
                println!(
                    "Archaic [--profile NAME] [--locale en|de] [--theme system|light|dark] [--fixture] [--smoke-test] [--temporary-session]\nDefault: real Matrix password sign-in. Fixture mode never connects."
                );
                return Ok(None);
            }
            _ => return Err(format!("Unknown option: {arg}").into()),
        }
    }
    Ok(Some(Options {
        locale,
        profile,
        explicit_profile,
        link,
        theme,
        fixture_mode,
        smoke,
        temporary,
    }))
}

impl Options {
    pub fn window_title(&self) -> &'static str {
        if self.fixture_mode {
            "Archaic — Offline UI preview"
        } else {
            "Archaic"
        }
    }
}
