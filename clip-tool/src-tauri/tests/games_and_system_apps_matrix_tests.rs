use app_lib::process_list::is_background_process;

// ──────────────────────────────────────────────────────────────────────────────
// Top Games Database (Must NOT be flagged as background)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_games_database_100_popular_titles() {
    let games = [
        ("cs2.exe", "D:\\Games\\Counter-Strike 2\\game\\bin\\win64\\cs2.exe"),
        ("dota2.exe", "D:\\Steam\\steamapps\\common\\dota 2 beta\\game\\bin\\win64\\dota2.exe"),
        ("valorant-win64-shipping.exe", "C:\\Riot Games\\VALORANT\\live\\ShooterGame\\Binaries\\Win64\\VALORANT-Win64-Shipping.exe"),
        ("league of legends.exe", "C:\\Riot Games\\League of Legends\\Game\\League of Legends.exe"),
        ("overwatch.exe", "C:\\Program Files (x86)\\Overwatch\\_retail_\\Overwatch.exe"),
        ("apexlegends.exe", "C:\\Program Files\\EA Games\\Apex Legends\\r5apex.exe"),
        ("fortniteclient-win64-shipping.exe", "C:\\Program Files\\Epic Games\\Fortnite\\FortniteGame\\Binaries\\Win64\\FortniteClient-Win64-Shipping.exe"),
        ("gta5.exe", "D:\\Rockstar Games\\Grand Theft Auto V\\GTA5.exe"),
        ("rdr2.exe", "D:\\Rockstar Games\\Red Dead Redemption 2\\RDR2.exe"),
        ("cyberpunk2077.exe", "C:\\GOG Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe"),
        ("witcher3.exe", "D:\\Games\\The Witcher 3\\bin\\x64\\witcher3.exe"),
        ("eldenring.exe", "D:\\Steam\\steamapps\\common\\ELDEN RING\\Game\\eldenring.exe"),
        ("sekiro.exe", "D:\\Steam\\steamapps\\common\\Sekiro\\sekiro.exe"),
        ("dark_souls_3.exe", "D:\\Steam\\steamapps\\common\\DARK SOULS III\\Game\\DarkSoulsIII.exe"),
        ("armoredcore6.exe", "D:\\Steam\\steamapps\\common\\ARMORED CORE VI FIRES OF RUBICON\\Game\\armoredcore6.exe"),
        ("helldivers2.exe", "D:\\Steam\\steamapps\\common\\HELLDIVERS 2\\bin\\helldivers2.exe"),
        ("palworld-win64-shipping.exe", "D:\\Steam\\steamapps\\common\\Palworld\\Pal\\Binaries\\Win64\\Palworld-Win64-Shipping.exe"),
        ("bg3.exe", "D:\\Steam\\steamapps\\common\\Baldurs Gate 3\\bin\\bg3.exe"),
        ("bg3_dx11.exe", "D:\\Steam\\steamapps\\common\\Baldurs Gate 3\\bin\\bg3_dx11.exe"),
        ("starfield.exe", "D:\\XboxGames\\Starfield\\Content\\Starfield.exe"),
        ("skyrimse.exe", "D:\\Steam\\steamapps\\common\\Skyrim Special Edition\\SkyrimSE.exe"),
        ("fallout4.exe", "D:\\Steam\\steamapps\\common\\Fallout 4\\Fallout4.exe"),
        ("rocketleague.exe", "C:\\Program Files\\Epic Games\\rocketleague\\Binaries\\Win64\\RocketLeague.exe"),
        ("rainbowsix.exe", "D:\\Ubisoft\\Ubisoft Game Launcher\\games\\Tom Clancy's Rainbow Six Siege\\RainbowSix.exe"),
        ("tslgame.exe", "D:\\Steam\\steamapps\\common\\PUBG\\TslGame\\Binaries\\Win64\\TslGame.exe"),
        ("rustclient.exe", "D:\\Steam\\steamapps\\common\\Rust\\RustClient.exe"),
        ("deadbydaylight-win64-shipping.exe", "D:\\Steam\\steamapps\\common\\Dead by Daylight\\DeadByDaylight\\Binaries\\Win64\\DeadByDaylight-Win64-Shipping.exe"),
        ("escapefromtarkov.exe", "C:\\Battlestate Games\\BsgLauncher\\Games\\EscapeFromTarkov.exe"),
        ("huntgame.exe", "D:\\Steam\\steamapps\\common\\Hunt Showdown\\bin\\win_x64\\HuntGame.exe"),
        ("destiny2.exe", "D:\\Steam\\steamapps\\common\\Destiny 2\\destiny2.exe"),
        ("warframe.x64.exe", "D:\\Steam\\steamapps\\common\\Warframe\\Warframe.x64.exe"),
        ("monstertrainer.exe", "D:\\Steam\\steamapps\\common\\Monster Hunter World\\MonsterHunterWorld.exe"),
        ("forzahorizon5.exe", "D:\\XboxGames\\Forza Horizon 5\\Content\\ForzaHorizon5.exe"),
        ("f1_24.exe", "D:\\Steam\\steamapps\\common\\F1 24\\F1_24.exe"),
        ("ea_fc25.exe", "C:\\Program Files\\EA Games\\EA SPORTS FC 25\\FC25.exe"),
        ("nba2k25.exe", "D:\\Steam\\steamapps\\common\\NBA 2K25\\NBA2K25.exe"),
        ("streetfighter6.exe", "D:\\Steam\\steamapps\\common\\Street Fighter 6\\StreetFighter6.exe"),
        ("tekken8.exe", "D:\\Steam\\steamapps\\common\\TEKKEN 8\\Polaris\\Binaries\\Win64\\Polaris-Win64-Shipping.exe"),
        ("guiltygearstrive.exe", "D:\\Steam\\steamapps\\common\\GUILTY GEAR STRIVE\\GGST.exe"),
        ("brawlhalla.exe", "D:\\Steam\\steamapps\\common\\Brawlhalla\\Brawlhalla.exe"),
        ("smite2.exe", "D:\\Steam\\steamapps\\common\\SMITE 2\\SMITE2.exe"),
        ("genshinimpact.exe", "C:\\Program Files\\Genshin Impact\\Genshin Impact game\\GenshinImpact.exe"),
        ("honkaistarrail.exe", "C:\\Program Files\\Star Rail\\Games\\StarRail.exe"),
        ("zenlesszonezero.exe", "C:\\Program Files\\ZenlessZoneZero Game\\ZenlessZoneZero.exe"),
        ("wutheringwaves.exe", "C:\\Wuthering Waves\\Wuthering Waves Game\\Wuthering Waves.exe"),
        ("minecraft.exe", "C:\\Users\\Game\\AppData\\Roaming\\.minecraft\\runtime\\java-runtime\\bin\\javaw.exe"),
        ("terraria.exe", "D:\\Steam\\steamapps\\common\\Terraria\\Terraria.exe"),
        ("stardewvalley.exe", "D:\\Steam\\steamapps\\common\\Stardew Valley\\Stardew Valley.exe"),
        ("subnautica.exe", "D:\\Steam\\steamapps\\common\\Subnautica\\Subnautica.exe"),
        ("hollowknight.exe", "D:\\Steam\\steamapps\\common\\Hollow Knight\\hollow_knight.exe"),
    ];

    for (name, path) in games {
        assert!(
            !is_background_process(name, path),
            "Game '{}' was falsely classified as a background process!",
            name
        );
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// System Core Services Matrix (MUST be classified as background)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_system_core_services_exhaustive() {
    let services = [
        ("svchost.exe", "C:\\Windows\\System32\\svchost.exe"),
        ("dwm.exe", "C:\\Windows\\System32\\dwm.exe"),
        ("csrss.exe", "C:\\Windows\\System32\\csrss.exe"),
        ("lsass.exe", "C:\\Windows\\System32\\lsass.exe"),
        ("winlogon.exe", "C:\\Windows\\System32\\winlogon.exe"),
        ("services.exe", "C:\\Windows\\System32\\services.exe"),
        ("smss.exe", "C:\\Windows\\System32\\smss.exe"),
        ("wininit.exe", "C:\\Windows\\System32\\wininit.exe"),
        ("spoolsv.exe", "C:\\Windows\\System32\\spoolsv.exe"),
        ("taskhostw.exe", "C:\\Windows\\System32\\taskhostw.exe"),
        ("audiodg.exe", "C:\\Windows\\System32\\audiodg.exe"),
        ("searchindexer.exe", "C:\\Windows\\System32\\SearchIndexer.exe"),
        ("searchhost.exe", "C:\\Windows\\SystemApps\\Microsoft.Windows.Search_cw5n1h2txyewy\\SearchHost.exe"),
        ("runtimebroker.exe", "C:\\Windows\\System32\\RuntimeBroker.exe"),
        ("fontdrvhost.exe", "C:\\Windows\\System32\\fontdrvhost.exe"),
        ("sihost.exe", "C:\\Windows\\System32\\sihost.exe"),
        ("ctfmon.exe", "C:\\Windows\\System32\\ctfmon.exe"),
        ("dllhost.exe", "C:\\Windows\\System32\\dllhost.exe"),
        ("conhost.exe", "C:\\Windows\\System32\\conhost.exe"),
        ("backgroundtaskhost.exe", "C:\\Windows\\System32\\BackgroundTaskHost.exe"),
        ("wermgr.exe", "C:\\Windows\\System32\\wermgr.exe"),
        ("werfault.exe", "C:\\Windows\\System32\\WerFault.exe"),
        ("vssvc.exe", "C:\\Windows\\System32\\vssvc.exe"),
        ("wmiprvse.exe", "C:\\Windows\\System32\\wbem\\WmiPrvSE.exe"),
        ("tiworker.exe", "C:\\Windows\\WinSxS\\amd64_microsoft-windows-servicingstack_none_\\TiWorker.exe"),
        ("trustedinstaller.exe", "C:\\Windows\\servicing\\TrustedInstaller.exe"),
    ];

    for (name, path) in services {
        assert!(
            is_background_process(name, path),
            "System service '{}' was NOT identified as a background process!",
            name
        );
    }
}
