use std::env;
use std::fs;
use std::path::Path;

pub fn current_username() -> String {
    env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| "root".to_string())
}

pub fn current_home_dir() -> String {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string())
}

pub fn expand_user_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{}", current_home_dir(), rest)
    } else {
        path.to_string()
    }
}

pub fn get_user_default_shell(username: &str) -> String {
    #[cfg(windows)]
    {
        let _ = username;
        return r"c:\windows\system32\windowspowershell\v1.0\powershell.exe".to_string();
    }

    #[cfg(not(windows))]
    {
        let fallback = "/bin/sh".to_string();
        let Ok(passwd) = fs::read_to_string("/etc/passwd") else {
            return fallback;
        };
        for line in passwd.lines() {
            let fields: Vec<_> = line.split(':').collect();
            if fields.len() == 7 && fields[0] == username {
                return fields[6].to_string();
            }
        }
        fallback
    }
}

pub fn byte_count_si(bytes: i64) -> String {
    const UNIT: i64 = 1000;
    if bytes < UNIT {
        return format!("{bytes} B");
    }

    let mut div = UNIT;
    let mut exp = 0usize;
    let mut n = bytes / UNIT;
    while n >= UNIT {
        div *= UNIT;
        exp += 1;
        n /= UNIT;
    }

    format!("{:.1} {}B", bytes as f64 / div as f64, ['k', 'M', 'G', 'T', 'P', 'E'][exp])
}

pub fn write_file_0600(path: &Path, contents: &[u8]) -> Result<(), String> {
    fs::write(path, contents).map_err(|err| err.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions).map_err(|err| err.to_string())?;
    }
    Ok(())
}
