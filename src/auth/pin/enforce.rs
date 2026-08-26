//! PIN method: exactly one pam_pwdfile line; anything else disables this method.

use std::path::{Path, PathBuf};

pub const SERVICE_NAME: &str = "mechanix-lockscreen";
pub const MODULE_NAME: &str = "pam_pwdfile.so";
pub const PIN_FILE: &str = "/etc/mechanix/pin.passwd";
pub const CONTROL: &str = "required";

pub const SERVICE_DIRS: &[&str] = &["/etc/pam.d", "/usr/lib/pam.d"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceConfig {
    pub path: PathBuf,
    pub pwdfile: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    NotFound,
    Invalid(String),
    Io(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound => write!(
                f,
                "PAM service '{SERVICE_NAME}' not found in {}",
                SERVICE_DIRS.join(", ")
            ),
            ConfigError::Invalid(detail) => {
                write!(f, "PAM service '{SERVICE_NAME}' rejected: {detail}")
            }
            ConfigError::Io(detail) => write!(f, "PAM service unreadable: {detail}"),
        }
    }
}

fn module_basename(module: &str) -> &str {
    module.rsplit('/').next().unwrap_or(module)
}

pub fn validate_content(content: &str) -> Result<ServiceConfig, ConfigError> {
    let mut pwdfile: Option<String> = None;

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if pwdfile.is_some() {
            return Err(ConfigError::Invalid(
                "more than one active stack line".into(),
            ));
        }

        let mut fields = line.split_whitespace();
        let stack_type = fields.next();
        let control = fields.next();
        let module = fields.next();
        let args: Vec<&str> = fields.collect();

        let Some(stack_type) = stack_type else {
            continue;
        };
        let Some(control) = control else {
            return Err(ConfigError::Invalid(format!(
                "line without control: '{line}'"
            )));
        };
        let Some(module) = module else {
            return Err(ConfigError::Invalid(format!(
                "line without module: '{line}'"
            )));
        };

        if stack_type != "auth" {
            return Err(ConfigError::Invalid(format!(
                "only 'auth' stack lines are enforced, found '{stack_type}'"
            )));
        }

        if control != CONTROL {
            return Err(ConfigError::Invalid(format!(
                "control must be '{CONTROL}', found '{control}'"
            )));
        }
        if module_basename(module) != MODULE_NAME {
            return Err(ConfigError::Invalid(format!(
                "module must be {MODULE_NAME}, found '{module}'"
            )));
        }
        if args.len() != 1 {
            return Err(ConfigError::Invalid(format!(
                "expected exactly one argument (pwdfile=), found {args:?}"
            )));
        }
        let Some(arg_path) = args[0].strip_prefix("pwdfile=") else {
            return Err(ConfigError::Invalid(format!(
                "expected 'pwdfile=…' argument, found '{}'",
                args[0]
            )));
        };
        if arg_path != PIN_FILE {
            return Err(ConfigError::Invalid(format!(
                "pwdfile must be {PIN_FILE}, found '{arg_path}'"
            )));
        }
        pwdfile = Some(arg_path.to_string());
    }

    match pwdfile {
        Some(pwdfile) => Ok(ServiceConfig {
            path: PathBuf::from(PIN_FILE),
            pwdfile,
        }),
        None => Err(ConfigError::Invalid("no active stack lines".into())),
    }
}

pub fn validate_installed() -> Result<ServiceConfig, ConfigError> {
    let mut last_err = ConfigError::NotFound;
    for dir in SERVICE_DIRS {
        let path = Path::new(dir).join(SERVICE_NAME);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                last_err = ConfigError::Io(format!("{}: {e}", path.display()));
                continue;
            }
        };
        return validate_content(&content).map_err(|e| match e {
            ConfigError::Invalid(d) => ConfigError::Invalid(format!("{d} ({})", path.display())),
            other => other,
        });
    }
    Err(last_err)
}

/// True when the book contains a `{user}:` entry, the shape make-pin.sh writes.
pub fn pin_listed(path: &Path, user: &str) -> bool {
    match std::fs::read_to_string(path) {
        Ok(content) => book_lists(&content, user),
        Err(_) => false,
    }
}

fn book_lists(content: &str, user: &str) -> bool {
    let prefix = format!("{user}:");
    content.lines().any(|line| line.starts_with(&prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_full_module_path() {
        let content =
            "auth required /usr/lib64/security/pam_pwdfile.so pwdfile=/etc/mechanix/pin.passwd\n";
        assert!(validate_content(content).is_ok());
    }

    #[test]
    fn book_lists_matches_user_line_only() {
        let book = "# comment\nmecha:$6$salt$hash\nother:$6$x$y\n";
        assert!(book_lists(book, "mecha"));
        assert!(book_lists(book, "other"));
        assert!(!book_lists(book, "nobody"));
        assert!(!book_lists("mechaX:$6$a$b\n", "mecha"));
        assert!(!book_lists("", "mecha"));
    }
}
