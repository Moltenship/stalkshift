use stalkshift_protocol::CruiseUnit;
use std::ffi::CStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Game {
    Ets2,
    Ats,
}
impl Game {
    pub fn from_id(id: &CStr) -> Option<Self> {
        match id.to_bytes() {
            b"eut2" => Some(Self::Ets2),
            b"ats" => Some(Self::Ats),
            _ => None,
        }
    }
    pub fn default_unit(self) -> CruiseUnit {
        match self {
            Self::Ets2 => CruiseUnit::Kmh,
            Self::Ats => CruiseUnit::Mph,
        }
    }
    pub fn cruise_unit(self, setting: Option<&str>) -> CruiseUnit {
        match setting.map(str::trim) {
            Some("kmh") => CruiseUnit::Kmh,
            Some("mph") => CruiseUnit::Mph,
            _ => self.default_unit(),
        }
    }
    pub fn installed_unit(self) -> CruiseUnit {
        // current_exe is the game executable. The installer writes this setting
        // beside our plugin, independent of the game's working directory.
        let setting = std::env::current_exe().ok().and_then(|exe| {
            std::fs::read_to_string(exe.parent()?.join("plugins/stalkshift-cruise-unit.txt")).ok()
        });
        self.cruise_unit(setting.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_documented_game_ids_are_accepted() {
        assert_eq!(Game::from_id(c"eut2"), Some(Game::Ets2));
        assert_eq!(Game::from_id(c"ats"), Some(Game::Ats));
        for id in [c"", c"amtrucks", c"other", c"ATS"] {
            assert_eq!(Game::from_id(id), None);
        }
    }
    #[test]
    fn unit_settings_override_each_games_default() {
        for game in [Game::Ets2, Game::Ats] {
            assert_eq!(game.cruise_unit(Some("mph\r\n")), CruiseUnit::Mph);
            assert_eq!(game.cruise_unit(Some("kmh\n")), CruiseUnit::Kmh);
            assert_eq!(game.cruise_unit(None), game.default_unit());
            assert_eq!(game.cruise_unit(Some("invalid")), game.default_unit());
        }
    }
}
