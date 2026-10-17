use openusage_codex::{AccountAuth, IdentityConfidence, session::probe_account};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() != 2 {
        return Err(
            "Supply an approved absolute executable and a private managed directory.".into(),
        );
    }
    let probe = probe_account(&PathBuf::from(&arguments[0]), &PathBuf::from(&arguments[1]))?;
    if probe.account.authentication != AccountAuth::AuthenticationRequired
        || probe.account.identity != IdentityConfidence::Unknown
        || probe.account.email.is_some()
        || probe.account.plan.is_some()
    {
        return Err("The fresh isolated provider returned unexpected authentication.".into());
    }
    println!(
        "probe=passed version={} authentication=required identity=unknown terminated_pid={}",
        probe.version, probe.process_id
    );
    Ok(())
}
