use app_lib::process_list::is_background_process;

#[test]
fn test_system_directories_lower() {
    assert!(is_background_process("service.exe", "c:\\windows\\system32\\service.exe"));
    assert!(is_background_process("driver.exe", "c:\\windows\\syswow64\\driver.exe"));
    assert!(is_background_process("component.exe", "c:\\windows\\winsxs\\msil_test.exe"));
}

#[test]
fn test_system_directories_mixed_casing() {
    assert!(is_background_process("service.exe", "C:\\Windows\\System32\\Service.exe"));
    assert!(is_background_process("driver.exe", "C:\\WINDOWS\\SYSWOW64\\DRIVER.EXE"));
    assert!(is_background_process("comp.exe", "C:\\Windows\\WinSxS\\something.exe"));
}

#[test]
fn test_system_core_services_svchost() { assert!(is_background_process("svchost.exe", "")); }
#[test]
fn test_system_core_services_dwm() { assert!(is_background_process("dwm.exe", "")); }
#[test]
fn test_system_core_services_audiodg() { assert!(is_background_process("audiodg.exe", "")); }
#[test]
fn test_system_core_services_csrss() { assert!(is_background_process("csrss.exe", "")); }
#[test]
fn test_system_core_services_smss() { assert!(is_background_process("smss.exe", "")); }
#[test]
fn test_system_core_services_wininit() { assert!(is_background_process("wininit.exe", "")); }
#[test]
fn test_system_core_services_winlogon() { assert!(is_background_process("winlogon.exe", "")); }
#[test]
fn test_system_core_services_lsass() { assert!(is_background_process("lsass.exe", "")); }
#[test]
fn test_system_core_services_spoolsv() { assert!(is_background_process("spoolsv.exe", "")); }
#[test]
fn test_system_core_services_services() { assert!(is_background_process("services.exe", "")); }
#[test]
fn test_system_core_services_registry() { assert!(is_background_process("registry", "")); }
#[test]
fn test_system_core_services_system() { assert!(is_background_process("system", "")); }
#[test]
fn test_system_core_services_idle() { assert!(is_background_process("idle", "")); }
#[test]
fn test_system_core_services_conhost() { assert!(is_background_process("conhost.exe", "")); }
#[test]
fn test_system_core_services_runtimebroker() { assert!(is_background_process("runtimebroker.exe", "")); }
#[test]
fn test_system_core_services_searchindexer() { assert!(is_background_process("searchindexer.exe", "")); }
#[test]
fn test_system_core_services_searchhost() { assert!(is_background_process("searchhost.exe", "")); }
#[test]
fn test_system_core_services_taskhostw() { assert!(is_background_process("taskhostw.exe", "")); }
#[test]
fn test_system_core_services_sihost() { assert!(is_background_process("sihost.exe", "")); }
#[test]
fn test_system_core_services_ctfmon() { assert!(is_background_process("ctfmon.exe", "")); }
#[test]
fn test_system_core_services_dllhost() { assert!(is_background_process("dllhost.exe", "")); }
#[test]
fn test_system_core_services_wermgr() { assert!(is_background_process("wermgr.exe", "")); }
#[test]
fn test_system_core_services_werfault() { assert!(is_background_process("werfault.exe", "")); }
#[test]
fn test_system_core_services_vssvc() { assert!(is_background_process("vssvc.exe", "")); }
#[test]
fn test_system_core_services_wmiprvse() { assert!(is_background_process("wmiprvse.exe", "")); }
#[test]
fn test_system_core_services_mssense() { assert!(is_background_process("mssense.exe", "")); }

#[test]
fn test_games_valorant() { assert!(!is_background_process("valorant.exe", "c:\\riot games\\valorant\\valorant.exe")); }
#[test]
fn test_games_cs2() { assert!(!is_background_process("cs2.exe", "d:\\steam\\steamapps\\common\\cs2.exe")); }
#[test]
fn test_games_league_of_legends() { assert!(!is_background_process("leagueclient.exe", "c:\\riot games\\league\\leagueclient.exe")); }
#[test]
fn test_games_gta5() { assert!(!is_background_process("gta5.exe", "d:\\games\\gta v\\gta5.exe")); }
#[test]
fn test_games_fortnite() { assert!(!is_background_process("fortniteclient-win64-shipping.exe", "c:\\epic\\fortnite\\fortnite.exe")); }
#[test]
fn test_games_apex_legends() { assert!(!is_background_process("r5apex.exe", "d:\\steam\\apex\\r5apex.exe")); }
#[test]
fn test_games_overwatch() { assert!(!is_background_process("overwatch.exe", "c:\\battle.net\\overwatch\\overwatch.exe")); }
#[test]
fn test_games_cyberpunk() { assert!(!is_background_process("cyberpunk2077.exe", "d:\\gog\\cyberpunk\\cyberpunk2077.exe")); }
#[test]
fn test_games_minecraft() { assert!(!is_background_process("javaw.exe", "c:\\users\\user\\appdata\\roaming\\.minecraft\\runtime\\bin\\javaw.exe")); }
#[test]
fn test_apps_discord() { assert!(!is_background_process("discord.exe", "c:\\users\\user\\appdata\\local\\discord\\discord.exe")); }
#[test]
fn test_apps_spotify() { assert!(!is_background_process("spotify.exe", "c:\\users\\user\\appdata\\roaming\\spotify\\spotify.exe")); }
#[test]
fn test_apps_obs() { assert!(!is_background_process("obs64.exe", "c:\\program files\\obs-studio\\bin\\64bit\\obs64.exe")); }
#[test]
fn test_apps_steam() { assert!(!is_background_process("steam.exe", "c:\\program files (x86)\\steam\\steam.exe")); }
#[test]
fn test_apps_chrome() { assert!(!is_background_process("chrome.exe", "c:\\program files\\google\\chrome\\application\\chrome.exe")); }
#[test]
fn test_apps_firefox() { assert!(!is_background_process("firefox.exe", "c:\\program files\\mozilla firefox\\firefox.exe")); }

// ──────────────────────────────────────────────────────────────────────────────
// Additional Modern Games Filtering Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test] fn test_games_elden_ring() { assert!(!is_background_process("eldenring.exe", "d:\\steam\\steamapps\\common\\elden ring\\game\\eldenring.exe")); }
#[test] fn test_games_baldurs_gate_3() { assert!(!is_background_process("bg3.exe", "d:\\steam\\steamapps\\common\\baldurs gate 3\\bin\\bg3.exe")); }
#[test] fn test_games_dota2() { assert!(!is_background_process("dota2.exe", "d:\\steam\\steamapps\\common\\dota 2 beta\\game\\bin\\win64\\dota2.exe")); }
#[test] fn test_games_helldivers_2() { assert!(!is_background_process("helldivers2.exe", "d:\\steam\\steamapps\\common\\helldivers 2\\bin\\helldivers2.exe")); }
#[test] fn test_games_rocket_league() { assert!(!is_background_process("rocketleague.exe", "c:\\epic games\\rocketleague\\binaries\\win64\\rocketleague.exe")); }
#[test] fn test_games_pubg() { assert!(!is_background_process("tslgame.exe", "d:\\steam\\steamapps\\common\\pubg\\tslgame\\binaries\\win64\\tslgame.exe")); }
#[test] fn test_games_rainbow_six_siege() { assert!(!is_background_process("rainbowsix.exe", "c:\\ubisoft\\games\\rainbow six siege\\rainbowsix.exe")); }
#[test] fn test_games_dead_by_daylight() { assert!(!is_background_process("deadbydaylight-win64-shipping.exe", "d:\\steam\\dead by daylight\\deadbydaylight.exe")); }
#[test] fn test_games_escape_from_tarkov() { assert!(!is_background_process("escapefromtarkov.exe", "c:\\battlestate games\\eft\\escapefromtarkov.exe")); }
#[test] fn test_games_world_of_warcraft() { assert!(!is_background_process("wow.exe", "c:\\program files (x86)\\world of warcraft\\_retail_\\wow.exe")); }

// ──────────────────────────────────────────────────────────────────────────────
// Additional Communication, Media & Browser Apps Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test] fn test_apps_slack() { assert!(!is_background_process("slack.exe", "c:\\users\\user\\appdata\\local\\slack\\slack.exe")); }
#[test] fn test_apps_ms_teams() { assert!(!is_background_process("ms-teams.exe", "c:\\program files\\windowsapps\\msteams.exe")); }
#[test] fn test_apps_telegram() { assert!(!is_background_process("telegram.exe", "c:\\users\\user\\appdata\\roaming\\telegram desktop\\telegram.exe")); }
#[test] fn test_apps_whatsapp() { assert!(!is_background_process("whatsapp.exe", "c:\\program files\\windowsapps\\whatsapp.exe")); }
#[test] fn test_apps_zoom() { assert!(!is_background_process("zoom.exe", "c:\\users\\user\\appdata\\roaming\\zoom\\bin\\zoom.exe")); }
#[test] fn test_apps_vlc() { assert!(!is_background_process("vlc.exe", "c:\\program files\\videolan\\vlc\\vlc.exe")); }
#[test] fn test_apps_foobar2000() { assert!(!is_background_process("foobar2000.exe", "c:\\program files\\foobar2000\\foobar2000.exe")); }
#[test] fn test_apps_msedge() { assert!(!is_background_process("msedge.exe", "c:\\program files (x86)\\microsoft\\edge\\application\\msedge.exe")); }
#[test] fn test_apps_brave() { assert!(!is_background_process("brave.exe", "c:\\program files\\bravesoftware\\brave-browser\\application\\brave.exe")); }
#[test] fn test_apps_opera() { assert!(!is_background_process("opera.exe", "c:\\users\\user\\appdata\\local\\programs\\opera\\opera.exe")); }

// ──────────────────────────────────────────────────────────────────────────────
// Windows System Daemons & Core Components Tests (Must Be Background)
// ──────────────────────────────────────────────────────────────────────────────

#[test] fn test_system_smartscreen() { assert!(is_background_process("smartscreen.exe", "c:\\windows\\system32\\smartscreen.exe")); }
#[test] fn test_system_fontdrvhost() { assert!(is_background_process("fontdrvhost.exe", "c:\\windows\\system32\\fontdrvhost.exe")); }
#[test] fn test_system_tiworker() { assert!(is_background_process("tiworker.exe", "c:\\windows\\winsxs\\tiworker.exe")); }
#[test] fn test_system_trustedinstaller() { assert!(is_background_process("trustedinstaller.exe", "c:\\windows\\servicing\\trustedinstaller.exe")); }
#[test] fn test_system_shellexperiencehost() { assert!(is_background_process("shellexperiencehost.exe", "c:\\windows\\systemapps\\shellexperiencehost.exe")); }
#[test] fn test_system_startmenuexperiencehost() { assert!(is_background_process("startmenuexperiencehost.exe", "c:\\windows\\systemapps\\startmenuexperiencehost.exe")); }
#[test] fn test_system_textinputhost() { assert!(is_background_process("textinputhost.exe", "c:\\windows\\systemapps\\textinputhost.exe")); }
#[test] fn test_system_gamebarpresencewriter() { assert!(is_background_process("gamebarpresencewriter.exe", "c:\\windows\\system32\\gamebarpresencewriter.exe")); }

