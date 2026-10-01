pub struct Options {
    pub locale: String,
    pub theme: cui::sys::cui_theme,
    pub fixture_mode: bool,
    pub smoke: bool,
}

pub fn parse() -> Result<Option<Options>, Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut locale = "en".to_string();
    let mut theme = cui::sys::CUI_THEME_SYSTEM;
    let mut fixture_mode = false;
    let mut smoke = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--locale" => locale = args.next().ok_or("--locale requires en or de")?.clone(),
            "--theme" => {
                theme = match args.next().map(String::as_str) {
                    Some("system") => cui::sys::CUI_THEME_SYSTEM,
                    Some("light") => cui::sys::CUI_THEME_LIGHT,
                    Some("dark") => cui::sys::CUI_THEME_DARK,
                    _ => return Err("--theme requires system, light or dark".into()),
                }
            }
            "--fixture" => fixture_mode = true,
            "--smoke-test" => smoke = true,
            "--help" => {
                println!(
                    "Archaic [--locale en|de] [--theme system|light|dark] [--fixture] [--smoke-test]\nDefault: real Matrix password sign-in. Fixture mode never connects."
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
    }))
}
