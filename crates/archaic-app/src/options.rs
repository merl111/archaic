pub struct Options {
    pub locale: String,
    pub theme: Option<crate::appearance::Appearance>,
    pub fixture_mode: bool,
    pub smoke: bool,
    pub temporary: bool,
}

pub fn parse() -> Result<Option<Options>, Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut locale = "en".to_string();
    let mut theme = None;
    let mut fixture_mode = false;
    let mut smoke = false;
    let mut temporary = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
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
                    "Archaic [--locale en|de] [--theme system|light|dark] [--fixture] [--smoke-test] [--temporary-session]\nDefault: real Matrix password sign-in. Fixture mode never connects."
                );
                return Ok(None);
            }
            _ => return Err(format!("Unknown option: {arg}").into()),
        }
    }
    Ok(Some(Options {
        locale,
        theme,
        fixture_mode,
        smoke,
        temporary,
    }))
}
