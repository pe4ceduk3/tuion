//! Compact click-region identifiers. `bevy_tui_texture` stores hit regions
//! as `u64`; this enum is the whole "widget framework" on top of that.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitId {
    Nav(u8),
    HeaderConfig,
    FileRow(u16),
    FileCrumb,
    EditorLine(u16),
    EditorFold(u16),
    EditorCrumb(u16),
    ConfigLine(u16),
    CalDay(u16),
    CalTask(u16),
    AlarmRow(u16),
    TimerRow(u16),
    /// row, action (0 = start/pause, 1 = stop)
    TimerAction(u16, u8),
    FormField(u16),
    /// 0 = save/confirm, 1 = cancel
    FormButton(u8),
    HelpClose,
    ModalOk,
}

const TAG_NAV: u64 = 1;
const TAG_CONFIG: u64 = 2;
const TAG_FILE_ROW: u64 = 3;
const TAG_FILE_CRUMB: u64 = 4;
const TAG_EDITOR_LINE: u64 = 5;
const TAG_EDITOR_FOLD: u64 = 6;
const TAG_EDITOR_CRUMB: u64 = 7;
const TAG_CONFIG_LINE: u64 = 8;
const TAG_CAL_DAY: u64 = 9;
const TAG_CAL_TASK: u64 = 10;
const TAG_ALARM_ROW: u64 = 11;
const TAG_TIMER_ROW: u64 = 12;
const TAG_TIMER_ACTION: u64 = 13;
const TAG_FORM_FIELD: u64 = 14;
const TAG_FORM_BUTTON: u64 = 15;
const TAG_HELP_CLOSE: u64 = 16;
const TAG_MODAL_OK: u64 = 17;

impl From<HitId> for u64 {
    fn from(id: HitId) -> u64 {
        let (tag, payload) = match id {
            HitId::Nav(i) => (TAG_NAV, i as u64),
            HitId::HeaderConfig => (TAG_CONFIG, 0),
            HitId::FileRow(i) => (TAG_FILE_ROW, i as u64),
            HitId::FileCrumb => (TAG_FILE_CRUMB, 0),
            HitId::EditorLine(i) => (TAG_EDITOR_LINE, i as u64),
            HitId::EditorFold(i) => (TAG_EDITOR_FOLD, i as u64),
            HitId::EditorCrumb(i) => (TAG_EDITOR_CRUMB, i as u64),
            HitId::ConfigLine(i) => (TAG_CONFIG_LINE, i as u64),
            HitId::CalDay(i) => (TAG_CAL_DAY, i as u64),
            HitId::CalTask(i) => (TAG_CAL_TASK, i as u64),
            HitId::AlarmRow(i) => (TAG_ALARM_ROW, i as u64),
            HitId::TimerRow(i) => (TAG_TIMER_ROW, i as u64),
            HitId::TimerAction(row, action) => {
                (TAG_TIMER_ACTION, (row as u64) | ((action as u64) << 16))
            }
            HitId::FormField(i) => (TAG_FORM_FIELD, i as u64),
            HitId::FormButton(i) => (TAG_FORM_BUTTON, i as u64),
            HitId::HelpClose => (TAG_HELP_CLOSE, 0),
            HitId::ModalOk => (TAG_MODAL_OK, 0),
        };
        (tag << 56) | payload
    }
}

impl TryFrom<u64> for HitId {
    type Error = ();
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        let tag = value >> 56;
        let payload = value & 0x00FF_FFFF_FFFF_FFFF;
        let small = payload as u16;
        Ok(match tag {
            TAG_NAV => HitId::Nav(payload as u8),
            TAG_CONFIG => HitId::HeaderConfig,
            TAG_FILE_ROW => HitId::FileRow(small),
            TAG_FILE_CRUMB => HitId::FileCrumb,
            TAG_EDITOR_LINE => HitId::EditorLine(small),
            TAG_EDITOR_FOLD => HitId::EditorFold(small),
            TAG_EDITOR_CRUMB => HitId::EditorCrumb(small),
            TAG_CONFIG_LINE => HitId::ConfigLine(small),
            TAG_CAL_DAY => HitId::CalDay(small),
            TAG_CAL_TASK => HitId::CalTask(small),
            TAG_ALARM_ROW => HitId::AlarmRow(small),
            TAG_TIMER_ROW => HitId::TimerRow(small),
            TAG_TIMER_ACTION => HitId::TimerAction(small, ((payload >> 16) & 0xFF) as u8),
            TAG_FORM_FIELD => HitId::FormField(small),
            TAG_FORM_BUTTON => HitId::FormButton(payload as u8),
            TAG_HELP_CLOSE => HitId::HelpClose,
            TAG_MODAL_OK => HitId::ModalOk,
            _ => return Err(()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_variants() {
        let ids = [
            HitId::Nav(3),
            HitId::HeaderConfig,
            HitId::FileRow(17),
            HitId::FileCrumb,
            HitId::EditorLine(99),
            HitId::EditorFold(4),
            HitId::EditorCrumb(2),
            HitId::ConfigLine(8),
            HitId::CalDay(41),
            HitId::CalTask(5),
            HitId::AlarmRow(1),
            HitId::TimerRow(2),
            HitId::TimerAction(2, 1),
            HitId::FormField(3),
            HitId::FormButton(1),
            HitId::HelpClose,
            HitId::ModalOk,
        ];
        for id in ids {
            let raw: u64 = id.into();
            assert_eq!(HitId::try_from(raw), Ok(id));
        }
    }
}
