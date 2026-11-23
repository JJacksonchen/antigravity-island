use std::collections::HashSet;

use antigravity_island::quota::discovery::{
    build_candidate_endpoints, discover_daemon_processes_in_dir, parse_alive_pids,
    parse_daemon_json, parse_lsof_ports, parse_ps_output, ProcessKind,
};

#[test]
fn parses_app_ide_cli_processes_and_skips_tokenless_desktop() {
    let ps = "\
  101 /Applications/Antigravity IDE.app/Contents/Resources/app/extensions/antigravity/bin/language_server_macos_arm --app_data_dir antigravity-ide --csrf_token=ide-secret --extension_server_port 51111 --extension_server_csrf_token ext-tok
  102 /Applications/Antigravity.app/Contents/MacOS/language_server_macos_arm --app_data_dir antigravity --csrf_token app-secret --https_server_port=49203
  103 /Applications/Antigravity.app/Contents/MacOS/language_server_macos_arm --app_data_dir antigravity
  104 /opt/homebrew/bin/agy
  105 /usr/local/bin/some-unrelated-tool --csrf_token nope
  106 /Applications/Gemini.app/Contents/Resources/language_server_macos --csrf-token \"quoted-gemini-tok\" --port 49500
";

    let procs = parse_ps_output(ps);
    assert_eq!(procs.len(), 4);

    // Sorted by kind priority: App (102, 106) -> Cli (104) -> Ide (101)
    assert_eq!(procs[0].pid, 102);
    assert_eq!(procs[0].kind, ProcessKind::App);
    assert_eq!(procs[0].csrf_token, "app-secret");
    assert_eq!(procs[0].hint_ports, vec![49203]);

    assert_eq!(procs[1].pid, 106);
    assert_eq!(procs[1].kind, ProcessKind::App);
    assert_eq!(procs[1].csrf_token, "quoted-gemini-tok");
    assert_eq!(procs[1].hint_ports, vec![49500]);

    assert_eq!(procs[2].pid, 104);
    assert_eq!(procs[2].kind, ProcessKind::Cli);
    assert_eq!(procs[2].csrf_token, "");

    assert_eq!(procs[3].pid, 101);
    assert_eq!(procs[3].kind, ProcessKind::Ide);
    assert_eq!(procs[3].csrf_token, "ide-secret");
    assert_eq!(procs[3].extension_server_port, Some(51111));
    assert_eq!(
        procs[3].extension_server_csrf_token.as_deref(),
        Some("ext-tok")
    );
}

#[test]
fn parses_lsof_ports_deduped_across_ipv4_ipv6_and_wildcard() {
    let lsof = "\
COMMAND     PID USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
language_ 12345 mac   11u  IPv4 0x12345      0t0  TCP 127.0.0.1:49203 (LISTEN)
language_ 12345 mac   12u  IPv6 0x12346      0t0  TCP [::1]:49203 (LISTEN)
language_ 12345 mac   13u  IPv4 0x12347      0t0  TCP *:51200 (LISTEN)
";
    let ports = parse_lsof_ports(lsof);
    assert_eq!(ports, vec![49203, 51200]);
}

#[test]
fn builds_ordered_candidate_endpoints_with_extension_fallback() {
    let ps = "200 /Applications/Antigravity.app/Contents/MacOS/language_server_macos_arm --app_data_dir antigravity --csrf_token ls-tok --extension_server_port 51111 --extension_server_csrf_token ext-tok";
    let procs = parse_ps_output(ps);
    let candidates = build_candidate_endpoints(&procs[0], &[49203, 51111]);

    assert_eq!(candidates.len(), 4);
    assert_eq!(candidates[0].scheme, "https");
    assert_eq!(candidates[0].port, 49203);
    assert_eq!(candidates[0].csrf_token, "ls-tok");
    assert!(!candidates[0].is_extension_server);

    assert_eq!(candidates[1].scheme, "http");
    assert_eq!(candidates[1].port, 49203);

    assert_eq!(candidates[2].scheme, "http");
    assert_eq!(candidates[2].port, 51111);
    assert_eq!(candidates[2].csrf_token, "ext-tok");
    assert!(candidates[2].is_extension_server);

    assert_eq!(candidates[3].port, 51111);
    assert_eq!(candidates[3].csrf_token, "ls-tok");
}

#[test]
fn parses_daemon_json_and_filters_stale_pids() {
    let dir = tempfile::tempdir().unwrap();
    let live_file = dir.path().join("hub_54008.json");
    let stale_file = dir.path().join("hub_51000.json");
    let invalid_file = dir.path().join("broken.json");

    std::fs::write(
        &live_file,
        r#"{"pid":46654,"httpsPort":54008,"httpPort":54007,"lsVersion":"2.34.0","csrfToken":"tok-live"}"#,
    )
    .unwrap();
    std::fs::write(
        &stale_file,
        r#"{"pid":99999,"httpsPort":51000,"httpPort":51001,"csrfToken":"tok-stale"}"#,
    )
    .unwrap();
    std::fs::write(
        &invalid_file,
        r#"{"pid":46654,"httpsPort":0,"csrfToken":""}"#,
    )
    .unwrap();

    let rec = parse_daemon_json(&std::fs::read(&live_file).unwrap()).unwrap();
    assert_eq!(rec.pid, 46654);
    assert_eq!(rec.https_port, Some(54008));
    assert_eq!(rec.http_port, Some(54007));

    let alive: HashSet<u32> = parse_alive_pids(
        "46654 /Applications/Antigravity.app/Contents/Resources/bin/language_server\n",
    );
    let procs = discover_daemon_processes_in_dir(dir.path(), ProcessKind::App, &alive);
    assert_eq!(procs.len(), 1);
    assert_eq!(procs[0].pid, 46654);
    assert_eq!(procs[0].csrf_token, "tok-live");
    assert_eq!(procs[0].explicit_https_port, Some(54008));
    assert_eq!(procs[0].explicit_http_port, Some(54007));

    let eps = build_candidate_endpoints(&procs[0], &[]);
    assert_eq!(eps.len(), 2);
    assert_eq!(eps[0].scheme, "https");
    assert_eq!(eps[0].port, 54008);
    assert_eq!(eps[1].scheme, "http");
    assert_eq!(eps[1].port, 54007);
}
