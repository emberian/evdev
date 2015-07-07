#![cfg_attr(feature = "unstable", feature(drain))]
#[macro_use]
extern crate bitflags;
extern crate ioctl;
extern crate libc;
extern crate errno;
extern crate fixedbitset;
extern crate num;

use std::os::unix::io::*;
use std::os::unix::ffi::*;
use std::path::Path;
use std::ffi::CString;
use std::mem::size_of;
use fixedbitset::FixedBitSet;

#[derive(Debug)]
pub enum Error {
    NulError(std::ffi::NulError),
    LibcError(errno::Errno),
    IoctlError(&'static str, errno::Errno),
}

impl From<std::ffi::NulError> for Error {
    fn from(e: std::ffi::NulError) -> Error {
        Error::NulError(e)
    }
}

impl From<errno::Errno> for Error {
    fn from(e: errno::Errno) -> Error {
        Error::LibcError(e)
    }
}

macro_rules! do_ioctl {
    ($name:ident($($arg:expr),+)) => {{
        let rc = unsafe { ::ioctl::$name($($arg,)+) };
        if rc < 0 {
            return Err(Error::IoctlError(stringify!($name), errno::errno()))
        }
        rc
    }}
}

struct Fd(libc::c_int);
impl Drop for Fd {
    fn drop(&mut self) {
        unsafe { libc::close(self.0); }
    }
}
impl std::ops::Deref for Fd {
    type Target = libc::c_int;
    fn deref(&self) -> &libc::c_int {
        &self.0
    }
}

bitflags! {
    flags Types: u32 {
        const SYNCHRONIZATION = 1 << 0x00,
        const KEY = 1 << 0x01,
        const RELATIVE = 1 << 0x02,
        const ABSOLUTE = 1 << 0x03,
        const MISC = 1 << 0x04,
        const SWITCH = 1 << 0x05,
        const LED = 1 << 0x11,
        const SOUND = 1 << 0x12,
        const REPEAT = 1 << 0x14,
        const FORCEFEEDBACK = 1 << 0x15,
        const POWER = 1 << 0x16,
        const FORCEFEEDBACKSTATUS = 1 << 0x17,
    }
}
bitflags! {
    flags Props: u32 {
        const POINTER = 1 << 0x00,
        const DIRECT = 1 << 0x01,
        const BUTTONPAD = 1 << 0x02,
        const SEMI_MT = 1 << 0x03,
        const TOPBUTTONPAD = 1 << 0x04,
        const POINTING_STICK = 1 << 0x05,
        const ACCELEROETER = 1 << 0x06
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum Key {
    KEY_RESERVED =	0,
    KEY_ESC =		1,
    KEY_1 =		2,
    KEY_2 =		3,
    KEY_3 =		4,
    KEY_4 =		5,
    KEY_5 =		6,
    KEY_6 =		7,
    KEY_7 =		8,
    KEY_8 =		9,
    KEY_9 =		10,
    KEY_0 =		11,
    KEY_MINUS =	12,
    KEY_EQUAL =	13,
    KEY_BACKSPACE =	14,
    KEY_TAB =		15,
    KEY_Q =		16,
    KEY_W =		17,
    KEY_E =		18,
    KEY_R =		19,
    KEY_T =		20,
    KEY_Y =		21,
    KEY_U =		22,
    KEY_I =		23,
    KEY_O =		24,
    KEY_P =		25,
    KEY_LEFTBRACE =	26,
    KEY_RIGHTBRACE =	27,
    KEY_ENTER =	28,
    KEY_LEFTCTRL =	29,
    KEY_A =		30,
    KEY_S =		31,
    KEY_D =		32,
    KEY_F =		33,
    KEY_G =		34,
    KEY_H =		35,
    KEY_J =		36,
    KEY_K =		37,
    KEY_L =		38,
    KEY_SEMICOLON =	39,
    KEY_APOSTROPHE =	40,
    KEY_GRAVE =	41,
    KEY_LEFTSHIFT =	42,
    KEY_BACKSLASH =	43,
    KEY_Z =		44,
    KEY_X =		45,
    KEY_C =		46,
    KEY_V =		47,
    KEY_B =		48,
    KEY_N =		49,
    KEY_M =		50,
    KEY_COMMA =	51,
    KEY_DOT =		52,
    KEY_SLASH =	53,
    KEY_RIGHTSHIFT =	54,
    KEY_KPASTERISK =	55,
    KEY_LEFTALT =	56,
    KEY_SPACE =	57,
    KEY_CAPSLOCK =	58,
    KEY_F1 =		59,
    KEY_F2 =		60,
    KEY_F3 =		61,
    KEY_F4 =		62,
    KEY_F5 =		63,
    KEY_F6 =		64,
    KEY_F7 =		65,
    KEY_F8 =		66,
    KEY_F9 =		67,
    KEY_F10 =		68,
    KEY_NUMLOCK =	69,
    KEY_SCROLLLOCK =	70,
    KEY_KP7 =		71,
    KEY_KP8 =		72,
    KEY_KP9 =		73,
    KEY_KPMINUS =	74,
    KEY_KP4 =		75,
    KEY_KP5 =		76,
    KEY_KP6 =		77,
    KEY_KPPLUS =	78,
    KEY_KP1 =		79,
    KEY_KP2 =		80,
    KEY_KP3 =		81,
    KEY_KP0 =		82,
    KEY_KPDOT =	83,
    KEY_ZENKAKUHANKAKU =85,
    KEY_102ND =	86,
    KEY_F11 =		87,
    KEY_F12 =		88,
    KEY_RO =		89,
    KEY_KATAKANA =	90,
    KEY_HIRAGANA =	91,
    KEY_HENKAN =	92,
    KEY_KATAKANAHIRAGANA =93,
    KEY_MUHENKAN =	94,
    KEY_KPJPCOMMA =	95,
    KEY_KPENTER =	96,
    KEY_RIGHTCTRL =	97,
    KEY_KPSLASH =	98,
    KEY_SYSRQ =	99,
    KEY_RIGHTALT =	100,
    KEY_LINEFEED =	101,
    KEY_HOME =	102,
    KEY_UP =		103,
    KEY_PAGEUP =	104,
    KEY_LEFT =	105,
    KEY_RIGHT =	106,
    KEY_END =		107,
    KEY_DOWN =	108,
    KEY_PAGEDOWN =	109,
    KEY_INSERT =	110,
    KEY_DELETE =	111,
    KEY_MACRO =	112,
    KEY_MUTE =	113,
    KEY_VOLUMEDOWN =	114,
    KEY_VOLUMEUP =	115,
    KEY_POWER =	116	/* SC System Power Down */,
    KEY_KPEQUAL =	117,
    KEY_KPPLUSMINUS =	118,
    KEY_PAUSE =	119,
    KEY_SCALE =	120	/* AL Compiz Scale (Expose) */,
    KEY_KPCOMMA =	121,
    KEY_HANGEUL =	122,
    KEY_HANJA =	123,
    KEY_YEN =		124,
    KEY_LEFTMETA =	125,
    KEY_RIGHTMETA =	126,
    KEY_COMPOSE =	127,
    KEY_STOP =	128	/* AC Stop */,
    KEY_AGAIN =	129,
    KEY_PROPS =	130	/* AC Properties */,
    KEY_UNDO =	131	/* AC Undo */,
    KEY_FRONT =	132,
    KEY_COPY =	133	/* AC Copy */,
    KEY_OPEN =	134	/* AC Open */,
    KEY_PASTE =	135	/* AC Paste */,
    KEY_FIND =	136	/* AC Search */,
    KEY_CUT =		137	/* AC Cut */,
    KEY_HELP =	138	/* AL Integrated Help Center */,
    KEY_MENU =	139	/* Menu (show menu) */,
    KEY_CALC =	140	/* AL Calculator */,
    KEY_SETUP =	141,
    KEY_SLEEP =	142	/* SC System Sleep */,
    KEY_WAKEUP =	143	/* System Wake Up */,
    KEY_FILE =	144	/* AL Local Machine Browser */,
    KEY_SENDFILE =	145,
    KEY_DELETEFILE =	146,
    KEY_XFER =	147,
    KEY_PROG1 =	148,
    KEY_PROG2 =	149,
    KEY_WWW =		150	/* AL Internet Browser */,
    KEY_MSDOS =	151,
    KEY_COFFEE =	152	/* AL Terminal Lock/Screensaver */,
    KEY_DIRECTION =	153,
    KEY_CYCLEWINDOWS =154,
    KEY_MAIL =	155,
    KEY_BOOKMARKS =	156	/* AC Bookmarks */,
    KEY_COMPUTER =	157,
    KEY_BACK =	158	/* AC Back */,
    KEY_FORWARD =	159	/* AC Forward */,
    KEY_CLOSECD =	160,
    KEY_EJECTCD =	161,
    KEY_EJECTCLOSECD =162,
    KEY_NEXTSONG =	163,
    KEY_PLAYPAUSE =	164,
    KEY_PREVIOUSSONG =165,
    KEY_STOPCD =	166,
    KEY_RECORD =	167,
    KEY_REWIND =	168,
    KEY_PHONE =	169	/* Media Select Telephone */,
    KEY_ISO =		170,
    KEY_CONFIG =	171	/* AL Consumer Control Configuration */,
    KEY_HOMEPAGE =	172	/* AC Home */,
    KEY_REFRESH =	173	/* AC Refresh */,
    KEY_EXIT =	174	/* AC Exit */,
    KEY_MOVE =	175,
    KEY_EDIT =	176,
    KEY_SCROLLUP =	177,
    KEY_SCROLLDOWN =	178,
    KEY_KPLEFTPAREN =	179,
    KEY_KPRIGHTPAREN =180,
    KEY_NEW =		181	/* AC New */,
    KEY_REDO =	182	/* AC Redo/Repeat */,
    KEY_F13 =		183,
    KEY_F14 =		184,
    KEY_F15 =		185,
    KEY_F16 =		186,
    KEY_F17 =		187,
    KEY_F18 =		188,
    KEY_F19 =		189,
    KEY_F20 =		190,
    KEY_F21 =		191,
    KEY_F22 =		192,
    KEY_F23 =		193,
    KEY_F24 =		194,
    KEY_PLAYCD =	200,
    KEY_PAUSECD =	201,
    KEY_PROG3 =	202,
    KEY_PROG4 =	203,
    KEY_DASHBOARD =	204	/* AL Dashboard */,
    KEY_SUSPEND =	205,
    KEY_CLOSE =	206	/* AC Close */,
    KEY_PLAY =	207,
    KEY_FASTFORWARD =	208,
    KEY_BASSBOOST =	209,
    KEY_PRINT =	210	/* AC Print */,
    KEY_HP =		211,
    KEY_CAMERA =	212,
    KEY_SOUND =	213,
    KEY_QUESTION =	214,
    KEY_EMAIL =	215,
    KEY_CHAT =	216,
    KEY_SEARCH =	217,
    KEY_CONNECT =	218,
    KEY_FINANCE =	219,
    KEY_SPORT =	220,
    KEY_SHOP =	221,
    KEY_ALTERASE =	222,
    KEY_CANCEL =	223,
    KEY_BRIGHTNESSDOWN =224,
    KEY_BRIGHTNESSUP =225,
    KEY_MEDIA =	226,
    KEY_SWITCHVIDEOMODE = 227,
    KEY_KBDILLUMTOGGLE = 228,
    KEY_KBDILLUMDOWN = 229,
    KEY_KBDILLUMUP =	230,
    KEY_SEND =	231,
    KEY_REPLY =	232,
    KEY_FORWARDMAIL =	233,
    KEY_SAVE =	234,
    KEY_DOCUMENTS =	235,
    KEY_BATTERY =	236,
    KEY_BLUETOOTH =	237,
    KEY_WLAN =	238,
    KEY_UWB =		239,
    KEY_UNKNOWN =	240,
    KEY_VIDEO_NEXT =	241,
    KEY_VIDEO_PREV =	242,
    KEY_BRIGHTNESS_CYCLE =243,
    KEY_BRIGHTNESS_AUTO =244,
    KEY_DISPLAY_OFF =	245,
    KEY_WWAN =	246,
    KEY_RFKILL =	247,
    KEY_MICMUTE =	248,
    BTN_0 =		0x100,
    BTN_1 =		0x101,
    BTN_2 =		0x102,
    BTN_3 =		0x103,
    BTN_4 =		0x104,
    BTN_5 =		0x105,
    BTN_6 =		0x106,
    BTN_7 =		0x107,
    BTN_8 =		0x108,
    BTN_9 =		0x109,
    BTN_LEFT =	0x110,
    BTN_RIGHT =	0x111,
    BTN_MIDDLE =	0x112,
    BTN_SIDE =	0x113,
    BTN_EXTRA =	0x114,
    BTN_FORWARD =	0x115,
    BTN_BACK =	0x116,
    BTN_TASK =	0x117,
    BTN_TRIGGER =	0x120,
    BTN_THUMB =	0x121,
    BTN_THUMB2 =	0x122,
    BTN_TOP =		0x123,
    BTN_TOP2 =	0x124,
    BTN_PINKIE =	0x125,
    BTN_BASE =	0x126,
    BTN_BASE2 =	0x127,
    BTN_BASE3 =	0x128,
    BTN_BASE4 =	0x129,
    BTN_BASE5 =	0x12a,
    BTN_BASE6 =	0x12b,
    BTN_DEAD =	0x12f,
    BTN_SOUTH =	0x130,
    BTN_EAST =	0x131,
    BTN_C =		0x132,
    BTN_NORTH =	0x133,
    BTN_WEST =	0x134,
    BTN_Z =		0x135,
    BTN_TL =		0x136,
    BTN_TR =		0x137,
    BTN_TL2 =		0x138,
    BTN_TR2 =		0x139,
    BTN_SELECT =	0x13a,
    BTN_START =	0x13b,
    BTN_MODE =	0x13c,
    BTN_THUMBL =	0x13d,
    BTN_THUMBR =	0x13e,
    BTN_TOOL_PEN =	0x140,
    BTN_TOOL_RUBBER =	0x141,
    BTN_TOOL_BRUSH =	0x142,
    BTN_TOOL_PENCIL =	0x143,
    BTN_TOOL_AIRBRUSH =0x144,
    BTN_TOOL_FINGER =	0x145,
    BTN_TOOL_MOUSE =	0x146,
    BTN_TOOL_LENS =	0x147,
    BTN_TOOL_QUINTTAP =0x148	/* Five fingers on trackpad */,
    BTN_TOUCH =	0x14a,
    BTN_STYLUS =	0x14b,
    BTN_STYLUS2 =	0x14c,
    BTN_TOOL_DOUBLETAP =0x14d,
    BTN_TOOL_TRIPLETAP =0x14e,
    BTN_TOOL_QUADTAP =0x14f	/* Four fingers on trackpad */,
    BTN_GEAR_DOWN =	0x150,
    BTN_GEAR_UP =	0x151,
    KEY_OK =		0x160,
    KEY_SELECT =	0x161,
    KEY_GOTO =	0x162,
    KEY_CLEAR =	0x163,
    KEY_POWER2 =	0x164,
    KEY_OPTION =	0x165,
    KEY_INFO =	0x166	/* AL OEM Features/Tips/Tutorial */,
    KEY_TIME =	0x167,
    KEY_VENDOR =	0x168,
    KEY_ARCHIVE =	0x169,
    KEY_PROGRAM =	0x16a	/* Media Select Program Guide */,
    KEY_CHANNEL =	0x16b,
    KEY_FAVORITES =	0x16c,
    KEY_EPG =		0x16d,
    KEY_PVR =		0x16e	/* Media Select Home */,
    KEY_MHP =		0x16f,
    KEY_LANGUAGE =	0x170,
    KEY_TITLE =	0x171,
    KEY_SUBTITLE =	0x172,
    KEY_ANGLE =	0x173,
    KEY_ZOOM =	0x174,
    KEY_MODE =	0x175,
    KEY_KEYBOARD =	0x176,
    KEY_SCREEN =	0x177,
    KEY_PC =		0x178	/* Media Select Computer */,
    KEY_TV =		0x179	/* Media Select TV */,
    KEY_TV2 =		0x17a	/* Media Select Cable */,
    KEY_VCR =		0x17b	/* Media Select VCR */,
    KEY_VCR2 =	0x17c	/* VCR Plus */,
    KEY_SAT =		0x17d	/* Media Select Satellite */,
    KEY_SAT2 =	0x17e,
    KEY_CD =		0x17f	/* Media Select CD */,
    KEY_TAPE =	0x180	/* Media Select Tape */,
    KEY_RADIO =	0x181,
    KEY_TUNER =	0x182	/* Media Select Tuner */,
    KEY_PLAYER =	0x183,
    KEY_TEXT =	0x184,
    KEY_DVD =		0x185	/* Media Select DVD */,
    KEY_AUX =		0x186,
    KEY_MP3 =		0x187,
    KEY_AUDIO =	0x188	/* AL Audio Browser */,
    KEY_VIDEO =	0x189	/* AL Movie Browser */,
    KEY_DIRECTORY =	0x18a,
    KEY_LIST =	0x18b,
    KEY_MEMO =	0x18c	/* Media Select Messages */,
    KEY_CALENDAR =	0x18d,
    KEY_RED =		0x18e,
    KEY_GREEN =	0x18f,
    KEY_YELLOW =	0x190,
    KEY_BLUE =	0x191,
    KEY_CHANNELUP =	0x192	/* Channel Increment */,
    KEY_CHANNELDOWN =	0x193	/* Channel Decrement */,
    KEY_FIRST =	0x194,
    KEY_LAST =	0x195	/* Recall Last */,
    KEY_AB =		0x196,
    KEY_NEXT =	0x197,
    KEY_RESTART =	0x198,
    KEY_SLOW =	0x199,
    KEY_SHUFFLE =	0x19a,
    KEY_BREAK =	0x19b,
    KEY_PREVIOUS =	0x19c,
    KEY_DIGITS =	0x19d,
    KEY_TEEN =	0x19e,
    KEY_TWEN =	0x19f,
    KEY_VIDEOPHONE =	0x1a0	/* Media Select Video Phone */,
    KEY_GAMES =	0x1a1	/* Media Select Games */,
    KEY_ZOOMIN =	0x1a2	/* AC Zoom In */,
    KEY_ZOOMOUT =	0x1a3	/* AC Zoom Out */,
    KEY_ZOOMRESET =	0x1a4	/* AC Zoom */,
    KEY_WORDPROCESSOR =0x1a5	/* AL Word Processor */,
    KEY_EDITOR =	0x1a6	/* AL Text Editor */,
    KEY_SPREADSHEET =	0x1a7	/* AL Spreadsheet */,
    KEY_GRAPHICSEDITOR =0x1a8	/* AL Graphics Editor */,
    KEY_PRESENTATION =0x1a9	/* AL Presentation App */,
    KEY_DATABASE =	0x1aa	/* AL Database App */,
    KEY_NEWS =	0x1ab	/* AL Newsreader */,
    KEY_VOICEMAIL =	0x1ac	/* AL Voicemail */,
    KEY_ADDRESSBOOK =	0x1ad	/* AL Contacts/Address Book */,
    KEY_MESSENGER =	0x1ae	/* AL Instant Messaging */,
    KEY_DISPLAYTOGGLE =0x1af	/* Turn display (LCD) on and off */,
    KEY_SPELLCHECK =	0x1b0   /* AL Spell Check */,
    KEY_LOGOFF =	0x1b1   /* AL Logoff */,
    KEY_DOLLAR =	0x1b2,
    KEY_EURO =	0x1b3,
    KEY_FRAMEBACK =	0x1b4	/* Consumer - transport controls */,
    KEY_FRAMEFORWARD =0x1b5,
    KEY_CONTEXT_MENU =0x1b6	/* GenDesc - system context menu */,
    KEY_MEDIA_REPEAT =0x1b7	/* Consumer - transport control */,
    KEY_10CHANNELSUP =0x1b8	/* 10 channels up (10+) */,
    KEY_10CHANNELSDOWN =0x1b9	/* 10 channels down (10-) */,
    KEY_IMAGES =	0x1ba	/* AL Image Browser */,
    KEY_DEL_EOL =	0x1c0,
    KEY_DEL_EOS =	0x1c1,
    KEY_INS_LINE =	0x1c2,
    KEY_DEL_LINE =	0x1c3,
    KEY_FN =		0x1d0,
    KEY_FN_ESC =	0x1d1,
    KEY_FN_F1 =	0x1d2,
    KEY_FN_F2 =	0x1d3,
    KEY_FN_F3 =	0x1d4,
    KEY_FN_F4 =	0x1d5,
    KEY_FN_F5 =	0x1d6,
    KEY_FN_F6 =	0x1d7,
    KEY_FN_F7 =	0x1d8,
    KEY_FN_F8 =	0x1d9,
    KEY_FN_F9 =	0x1da,
    KEY_FN_F10 =	0x1db,
    KEY_FN_F11 =	0x1dc,
    KEY_FN_F12 =	0x1dd,
    KEY_FN_1 =	0x1de,
    KEY_FN_2 =	0x1df,
    KEY_FN_D =	0x1e0,
    KEY_FN_E =	0x1e1,
    KEY_FN_F =	0x1e2,
    KEY_FN_S =	0x1e3,
    KEY_FN_B =	0x1e4,
    KEY_BRL_DOT1 =	0x1f1,
    KEY_BRL_DOT2 =	0x1f2,
    KEY_BRL_DOT3 =	0x1f3,
    KEY_BRL_DOT4 =	0x1f4,
    KEY_BRL_DOT5 =	0x1f5,
    KEY_BRL_DOT6 =	0x1f6,
    KEY_BRL_DOT7 =	0x1f7,
    KEY_BRL_DOT8 =	0x1f8,
    KEY_BRL_DOT9 =	0x1f9,
    KEY_BRL_DOT10 =	0x1fa,
    KEY_NUMERIC_0 =	0x200	/* used by phones, remote controls, */,
    KEY_NUMERIC_1 =	0x201	/* and other keypads */,
    KEY_NUMERIC_2 =	0x202,
    KEY_NUMERIC_3 =	0x203,
    KEY_NUMERIC_4 =	0x204,
    KEY_NUMERIC_5 =	0x205,
    KEY_NUMERIC_6 =	0x206,
    KEY_NUMERIC_7 =	0x207,
    KEY_NUMERIC_8 =	0x208,
    KEY_NUMERIC_9 =	0x209,
    KEY_NUMERIC_STAR =0x20a,
    KEY_NUMERIC_POUND =0x20b,
    KEY_CAMERA_FOCUS =0x210,
    KEY_WPS_BUTTON =	0x211	/* WiFi Protected Setup key */,
    KEY_TOUCHPAD_TOGGLE =0x212	/* Request switch touchpad on or off */,
    KEY_TOUCHPAD_ON =	0x213,
    KEY_TOUCHPAD_OFF =0x214,
    KEY_CAMERA_ZOOMIN =0x215,
    KEY_CAMERA_ZOOMOUT =0x216,
    KEY_CAMERA_UP =	0x217,
    KEY_CAMERA_DOWN =	0x218,
    KEY_CAMERA_LEFT =	0x219,
    KEY_CAMERA_RIGHT =0x21a,
    KEY_ATTENDANT_ON =0x21b,
    KEY_ATTENDANT_OFF =0x21c,
    KEY_ATTENDANT_TOGGLE =0x21d	/* Attendant call on or off */,
    KEY_LIGHTS_TOGGLE =0x21e	/* Reading light on or off */,
    BTN_DPAD_UP =	0x220,
    BTN_DPAD_DOWN =	0x221,
    BTN_DPAD_LEFT =	0x222,
    BTN_DPAD_RIGHT =	0x223,
    KEY_ALS_TOGGLE =	0x230	/* Ambient light sensor */,
    KEY_BUTTONCONFIG =	0x240	/* AL Button Configuration */,
    KEY_TASKMANAGER =	0x241	/* AL Task/Project Manager */,
    KEY_JOURNAL =	0x242	/* AL Log/Journal/Timecard */,
    KEY_CONTROLPANEL =	0x243	/* AL Control Panel */,
    KEY_APPSELECT =	0x244	/* AL Select Task/Application */,
    KEY_SCREENSAVER =	0x245	/* AL Screen Saver */,
    KEY_VOICECOMMAND =	0x246	/* Listening Voice Command */,
    KEY_BRIGHTNESS_MIN =	0x250	/* Set Brightness to Minimum */,
    KEY_BRIGHTNESS_MAX =	0x251	/* Set Brightness to Maximum */,
    KEY_KBDINPUTASSIST_PREV =	0x260,
    KEY_KBDINPUTASSIST_NEXT =	0x261,
    KEY_KBDINPUTASSIST_PREVGROUP =	0x262,
    KEY_KBDINPUTASSIST_NEXTGROUP =	0x263,
    KEY_KBDINPUTASSIST_ACCEPT =	0x264,
    KEY_KBDINPUTASSIST_CANCEL =	0x265,
    BTN_TRIGGER_HAPPY1 =	0x2c0,
    BTN_TRIGGER_HAPPY2 =	0x2c1,
    BTN_TRIGGER_HAPPY3 =	0x2c2,
    BTN_TRIGGER_HAPPY4 =	0x2c3,
    BTN_TRIGGER_HAPPY5 =	0x2c4,
    BTN_TRIGGER_HAPPY6 =	0x2c5,
    BTN_TRIGGER_HAPPY7 =	0x2c6,
    BTN_TRIGGER_HAPPY8 =	0x2c7,
    BTN_TRIGGER_HAPPY9 =	0x2c8,
    BTN_TRIGGER_HAPPY10 =	0x2c9,
    BTN_TRIGGER_HAPPY11 =	0x2ca,
    BTN_TRIGGER_HAPPY12 =	0x2cb,
    BTN_TRIGGER_HAPPY13 =	0x2cc,
    BTN_TRIGGER_HAPPY14 =	0x2cd,
    BTN_TRIGGER_HAPPY15 =	0x2ce,
    BTN_TRIGGER_HAPPY16 =	0x2cf,
    BTN_TRIGGER_HAPPY17 =	0x2d0,
    BTN_TRIGGER_HAPPY18 =	0x2d1,
    BTN_TRIGGER_HAPPY19 =	0x2d2,
    BTN_TRIGGER_HAPPY20 =	0x2d3,
    BTN_TRIGGER_HAPPY21 =	0x2d4,
    BTN_TRIGGER_HAPPY22 =	0x2d5,
    BTN_TRIGGER_HAPPY23 =	0x2d6,
    BTN_TRIGGER_HAPPY24 =	0x2d7,
    BTN_TRIGGER_HAPPY25 =	0x2d8,
    BTN_TRIGGER_HAPPY26 =	0x2d9,
    BTN_TRIGGER_HAPPY27 =	0x2da,
    BTN_TRIGGER_HAPPY28 =	0x2db,
    BTN_TRIGGER_HAPPY29 =	0x2dc,
    BTN_TRIGGER_HAPPY30 =	0x2dd,
    BTN_TRIGGER_HAPPY31 =	0x2de,
    BTN_TRIGGER_HAPPY32 =	0x2df,
    BTN_TRIGGER_HAPPY33 =	0x2e0,
    BTN_TRIGGER_HAPPY34 =	0x2e1,
    BTN_TRIGGER_HAPPY35 =	0x2e2,
    BTN_TRIGGER_HAPPY36 =	0x2e3,
    BTN_TRIGGER_HAPPY37 =	0x2e4,
    BTN_TRIGGER_HAPPY38 =	0x2e5,
    BTN_TRIGGER_HAPPY39 =	0x2e6,
    BTN_TRIGGER_HAPPY40 =	0x2e7,
    KEY_MAX = 0x2ff,
}

bitflags! {
    flags RelativeAxis: u32 {
        const REL_X = 1 << 0x00,
        const REL_Y = 1 << 0x01,
        const REL_Z = 1 << 0x02,
        const REL_RX = 1 << 0x03,
        const REL_RY = 1 << 0x04,
        const REL_RZ = 1 << 0x05,
        const REL_HWHEEL = 1 << 0x06,
        const REL_DIAL = 1 << 0x07,
        const REL_WHEEL = 1 << 0x08,
        const REL_MISC = 1 << 0x09,
    }
}

bitflags! {
    flags AbsoluteAxis: u64 {
        const ABS_X = 1 << 0x00,
        const ABS_Y = 1 << 0x01,
        const ABS_Z = 1 << 0x02,
        const ABS_RX = 1 << 0x03,
        const ABS_RY = 1 << 0x04,
        const ABS_RZ = 1 << 0x05,
        const ABS_THROTTLE = 1 << 0x06,
        const ABS_RUDDER = 1 << 0x07,
        const ABS_WHEEL = 1 << 0x08,
        const ABS_GAS = 1 << 0x09,
        const ABS_BRAKE = 1 << 0x0a,
        const ABS_HAT0X = 1 << 0x10,
        const ABS_HAT0Y = 1 << 0x11,
        const ABS_HAT1X = 1 << 0x12,
        const ABS_HAT1Y = 1 << 0x13,
        const ABS_HAT2X = 1 << 0x14,
        const ABS_HAT2Y = 1 << 0x15,
        const ABS_HAT3X = 1 << 0x16,
        const ABS_HAT3Y = 1 << 0x17,
        const ABS_PRESSURE = 1 << 0x18,
        const ABS_DISTANCE = 1 << 0x19,
        const ABS_TILT_X = 1 << 0x1a,
        const ABS_TILT_Y = 1 << 0x1b,
        const ABS_TOOL_WIDTH = 1 << 0x1c,
        const ABS_VOLUME = 1 << 0x20,
        const ABS_MISC = 1 << 0x28,
        const ABS_MT_SLOT = 1 << 0x2f/* MT slot being modified */,
        const ABS_MT_TOUCH_MAJOR = 1 << 0x30/* Major axis of touching ellipse */,
        const ABS_MT_TOUCH_MINOR = 1 << 0x31/* Minor axis (omit if circular) */,
        const ABS_MT_WIDTH_MAJOR = 1 << 0x32/* Major axis of approaching ellipse */,
        const ABS_MT_WIDTH_MINOR = 1 << 0x33/* Minor axis (omit if circular) */,
        const ABS_MT_ORIENTATION = 1 << 0x34/* Ellipse orientation */,
        const ABS_MT_POSITION_X = 1 << 0x35/* Center X touch position */,
        const ABS_MT_POSITION_Y = 1 << 0x36/* Center Y touch position */,
        const ABS_MT_TOOL_TYPE = 1 << 0x37/* Type of touching device */,
        const ABS_MT_BLOB_ID = 1 << 0x38/* Group a set of packets as a blob */,
        const ABS_MT_TRACKING_ID = 1 << 0x39/* Unique ID of initiated contact */,
        const ABS_MT_PRESSURE = 1 << 0x3a/* Pressure on contact area */,
        const ABS_MT_DISTANCE = 1 << 0x3b/* Contact hover distance */,
        const ABS_MT_TOOL_X = 1 << 0x3c/* Center X tool position */,
        const ABS_MT_TOOL_Y = 1 << 0x3d/* Center Y tool position */,
        const ABS_MAX = 1 << 0x3f,
    }
}

bitflags! {
    flags Switch: u32 {
        const SW_LID = 1 << 0x00  /* set = lid shut */,
        const SW_TABLET_MODE = 1 << 0x01  /* set = tablet mode */,
        const SW_HEADPHONE_INSERT = 1 << 0x02  /* set = inserted */,
        const SW_RFKILL_ALL = 1 << 0x03  /* rfkill master switch, type "any" */,
        const SW_MICROPHONE_INSERT = 1 << 0x04  /* set = inserted */,
        const SW_DOCK = 1 << 0x05  /* set = plugged into dock */,
        const SW_LINEOUT_INSERT = 1 << 0x06  /* set = inserted */,
        const SW_JACK_PHYSICAL_INSERT = 1 << 0x07  /* set = mechanical switch set */,
        const SW_VIDEOOUT_INSERT = 1 << 0x08  /* set = inserted */,
        const SW_CAMERA_LENS_COVER = 1 << 0x09  /* set = lens covered */,
        const SW_KEYPAD_SLIDE = 1 << 0x0a  /* set = keypad slide out */,
        const SW_FRONT_PROXIMITY = 1 << 0x0b  /* set = front proximity sensor active */,
        const SW_ROTATE_LOCK = 1 << 0x0c  /* set = rotate locked/disabled */,
        const SW_LINEIN_INSERT = 1 << 0x0d  /* set = inserted */,
        const SW_MUTE_DEVICE = 1 << 0x0e  /* set = device disabled */,
        const SW_MAX = 1 << 0x0f,
    }
}

bitflags! {
    flags Led: u32 {
        const LED_NUML = 1 << 0x00,
        const LED_CAPSL = 1 << 0x01,
        const LED_SCROLLL = 1 << 0x02,
        const LED_COMPOSE = 1 << 0x03,
        const LED_KANA = 1 << 0x04,
        const LED_SLEEP = 1 << 0x05,
        const LED_SUSPEND = 1 << 0x06,
        const LED_MUTE = 1 << 0x07,
        const LED_MISC = 1 << 0x08,
        const LED_MAIL = 1 << 0x09,
        const LED_CHARGING = 1 << 0x0a,
        const LED_MAX = 1 << 0x0f,
    }
}

bitflags! {
    flags Misc: u32 {
        const MSC_SERIAL = 1 << 0x00,
        const MSC_PULSELED = 1 << 0x01,
        const MSC_GESTURE = 1 << 0x02,
        const MSC_RAW = 1 << 0x03,
        const MSC_SCAN = 1 << 0x04,
        const MSC_TIMESTAMP = 1 << 0x05,
        const MSC_MAX = 1 << 0x07,
    }
}

bitflags! {
    flags FFStatus: u32 {
        const FF_STATUS_STOPPED	= 1 << 0x00,
        const FF_STATUS_PLAYING	= 1 << 0x01,
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub enum FFEffect {
    FF_RUMBLE = 0x50,
    FF_PERIODIC = 0x51,
    FF_CONSTANT = 0x52,
    FF_SPRING = 0x53,
    FF_FRICTION = 0x54,
    FF_DAMPER = 0x55,
    FF_INERTIA = 0x56,
    FF_RAMP = 0x57,
    FF_SQUARE = 0x58,
    FF_TRIANGLE = 0x59,
    FF_SINE = 0x5a,
    FF_SAW_UP = 0x5b,
    FF_SAW_DOWN = 0x5c,
    FF_CUSTOM = 0x5d,
    FF_GAIN = 0x60,
    FF_AUTOCENTER = 0x61,
    FF_MAX = 0x7f,
}

bitflags! {
    flags Repeat: u32 {
        const REP_DELAY = 1 << 0x00,
        const REP_PERIOD = 1 << 0x01,
    }
}

bitflags! {
    flags Sound: u32 {
        const SND_CLICK = 1 << 0x00,
        const SND_BELL = 1 << 0x01,
        const SND_TONE = 1 << 0x02,
    }
}

pub struct Device {
    fd: RawFd,
    ty: Types,
    name: CString,
    phys: Option<CString>,
    uniq: Option<CString>,
    id: ioctl::input_id,
    props: Props,
    driver_version: (u8, u8, u8),
    key_bits: FixedBitSet,
    key_vals: FixedBitSet,
    rel: RelativeAxis,
    abs: AbsoluteAxis,
    abs_vals: Vec<ioctl::input_absinfo>,
    switch: Switch,
    switch_vals: FixedBitSet,
    led: Led,
    led_vals: FixedBitSet,
    misc: Misc,
    ff: FixedBitSet,
    ff_stat: FFStatus,
    rep: Repeat,
    snd: Sound,
    pending_events: Vec<ioctl::input_event>,
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut ds = f.debug_struct("Device");
        ds.field("name", &self.name).field("fd", &self.fd).field("ty", &self.ty);
        if let Some(ref phys) = self.phys {
            ds.field("phys", phys);
        }
        if let Some(ref uniq) = self.uniq {
            ds.field("uniq", uniq);
        }
        ds.field("id", &self.id)
          .field("id", &self.id)
          .field("props", &self.props)
          .field("driver_version", &self.driver_version);
        if self.ty.contains(SYNCHRONIZATION) {

        }
        if self.ty.contains(KEY) {
            ds.field("key_bits", &self.key_bits)
              .field("key_vals", &self.key_vals);
        }
        if self.ty.contains(RELATIVE) {
            ds.field("rel", &self.rel);
        }
        if self.ty.contains(ABSOLUTE) {
            ds.field("abs", &self.abs);
            for idx in (0..0x28) {
                let abs = 1 << idx;
                // ignore multitouch, we'll handle that later.
                if abs < ABS_MT_SLOT.bits() && self.abs.bits() & abs == 1 {
                    // eugh.
                    ds.field(&format!("abs_{:x}", idx), &self.abs_vals[idx as usize]);
                }
            }
        }
        if self.ty.contains(MISC) {

        }
        if self.ty.contains(SWITCH) {
            ds.field("switch", &self.switch)
              .field("switch_vals", &self.switch_vals);
        }
        if self.ty.contains(LED) {
            ds.field("led", &self.led)
              .field("led_vals", &self.led_vals);
        }
        if self.ty.contains(SOUND) {
            ds.field("snd", &self.snd);
        }
        if self.ty.contains(REPEAT) {
            ds.field("rep", &self.rep);
        }
        if self.ty.contains(FORCEFEEDBACK) {
            ds.field("ff", &self.ff);
        }
        if self.ty.contains(POWER) {
        }
        if self.ty.contains(FORCEFEEDBACKSTATUS) {
            ds.field("ff_stat", &self.ff_stat);
        }
        ds.finish()
    }
}

fn bus_name(x: u16) -> &'static str {
    match x {
        0x1 => "PCI",
        0x2 => "ISA Plug 'n Play",
        0x3 => "USB",
        0x4 => "HIL",
        0x5 => "Bluetooth",
        0x6 => "Virtual",
        0x10 => "ISA",
        0x11 => "i8042",
        0x12 => "XTKBD",
        0x13 => "RS232",
        0x14 => "Gameport",
        0x15 => "Parallel Port",
        0x16 => "Amiga",
        0x17 => "ADB",
        0x18 => "I2C",
        0x19 => "Host",
        0x1A => "GSC",
        0x1B => "Atari",
        0x1C => "SPI",
        _ => "Unknown",
    }
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        try!(writeln!(f, "{:?}", self.name));
        try!(writeln!(f, "  Driver version: {}.{}.{}", self.driver_version.0, self.driver_version.1, self.driver_version.2));
        if let Some(ref phys) = self.phys {
            try!(writeln!(f, "  Physical address: {:?}", phys));
        }
        if let Some(ref uniq) = self.uniq {
            try!(writeln!(f, "  Unique name: {:?}", uniq));
        }

        try!(writeln!(f, "  Bus: {}", bus_name(self.id.bustype)));
        try!(writeln!(f, "  Vendor: 0x{:x}", self.id.vendor));
        try!(writeln!(f, "  Product: 0x{:x}", self.id.product));
        try!(writeln!(f, "  Version: 0x{:x}", self.id.version));
        try!(writeln!(f, "  Properties: {:?}", self.props));

        if self.ty.contains(SYNCHRONIZATION) {

        }

        if self.ty.contains(KEY) {
            try!(writeln!(f, "  Keys supported:"));
            for key_idx in (0..self.key_bits.len()) {
                if self.key_bits.contains(key_idx) {
                    // Cross our fingers...
                    try!(writeln!(f, "    {:?} ({}index {})",
                                 unsafe { std::mem::transmute::<_, Key>(key_idx as libc::c_int) },
                                 if self.key_vals.contains(key_idx) { "pressed, " } else { "" },
                                 key_idx));
                }
            }
        }
        if self.ty.contains(RELATIVE) {
            try!(writeln!(f, "  Relative Axes: {:?}", self.rel));
        }
        if self.ty.contains(ABSOLUTE) {
            try!(writeln!(f, "  Absolute Axes:"));
            for idx in (0..0x28) {
                let abs = 1 << idx;
                // ignore multitouch, we'll handle that later.
                if abs < ABS_MT_SLOT.bits() && self.abs.bits() & abs == 1 {
                    // FIXME: abs val Debug is gross
                    try!(writeln!(f, "    {:?} ({:?}, index {})",
                         AbsoluteAxis::from_bits(abs).unwrap(),
                         self.abs_vals[idx as usize],
                         idx));
                }
            }
        }
        if self.ty.contains(MISC) {
            try!(writeln!(f, "  Miscellaneous capabilities: {:?}", self.misc));
        }
        if self.ty.contains(SWITCH) {
            try!(writeln!(f, "  Switches:"));
            for idx in (0..0xf) {
                let sw = 1 << idx;
                if sw < SW_MAX.bits() && self.switch.bits() & sw == 1 {
                    try!(writeln!(f, "    {:?} ({:?}, index {})",
                         Switch::from_bits(sw).unwrap(),
                         self.switch_vals[idx as usize],
                         idx));
                }
            }
        }
        if self.ty.contains(LED) {
            try!(writeln!(f, "  LEDs:"));
            for idx in (0..0xf) {
                let led = 1 << idx;
                if led < LED_MAX.bits() && self.led.bits() & led == 1 {
                    try!(writeln!(f, "    {:?} ({:?}, index {})",
                         Led::from_bits(led).unwrap(),
                         self.led_vals[idx as usize],
                         idx));
                }
            }
        }
        if self.ty.contains(SOUND) {
            try!(writeln!(f, "  Sound: {:?}", self.snd));
        }
        if self.ty.contains(REPEAT) {
            try!(writeln!(f, "  Repeats: {:?}", self.rep));
        }
        if self.ty.contains(FORCEFEEDBACK) {
            try!(writeln!(f, "  Force Feedback supported"));
        }
        if self.ty.contains(POWER) {
            try!(writeln!(f, "  Power supported"));
        }
        if self.ty.contains(FORCEFEEDBACKSTATUS) {
            try!(writeln!(f, "  Force Feedback status supported"));
        }
        Ok(())
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd); } // yes yes I know EINTR, close(2) isn't portable etc.
    }
}

fn ffs<T: num::FromPrimitive>(x: u32) -> T {
    T::from_u32(31 - x.leading_zeros()).unwrap()
}

impl Device {
    pub fn fd(&self) -> RawFd {
        self.fd
    }

    pub fn events_supported(&self) -> Types {
        self.ty
    }

    pub fn name(&self) -> &CString {
        &self.name
    }

    pub fn physical_path(&self) -> &Option<CString> {
        &self.phys
    }

    pub fn unique_name(&self) -> &Option<CString> {
        &self.uniq
    }

    pub fn input_id(&self) -> ioctl::input_id {
        self.id
    }

    pub fn properties(&self) -> Props {
        self.props
    }

    pub fn driver_version(&self) -> (u8, u8, u8) {
        self.driver_version
    }

    pub fn keys_supported(&self) -> &FixedBitSet {
        &self.key_bits
    }

    pub fn keys_pressed(&self) -> &FixedBitSet {
        &self.key_vals
    }

    pub fn relative_axes_supported(&self) -> RelativeAxis {
        self.rel
    }

    pub fn absolute_axes_supported(&self) -> AbsoluteAxis {
        self.abs
    }

    pub fn absolute_axes_values(&self) -> &[ioctl::input_absinfo] {
        &self.abs_vals
    }

    pub fn switches_supported(&self) -> Switch {
        self.switch
    }

    pub fn switches_pressed(&self) -> &FixedBitSet {
        &self.switch_vals
    }

    pub fn leds_supported(&self) -> Led {
        self.led
    }

    pub fn leds_lit(&self) -> &FixedBitSet {
        &self.led_vals
    }

    pub fn misc_properties(&self) -> Misc {
        self.misc
    }

    pub fn repeats_supported(&self) -> Repeat {
        self.rep
    }

    pub fn sounds_supported(&self) -> Sound {
        self.snd
    }

    pub fn open(path: &AsRef<Path>) -> Result<Device, Error> {
        let cstr = match CString::new(path.as_ref().as_os_str().as_bytes()) {
            Ok(s) => s,
            Err(e) => return Err(Error::NulError(e))
        };
        // FIXME: only need for writing is for setting LED values. re-evaluate always using RDWR
        // later.
        let fd = Fd(unsafe { libc::open(cstr.as_ptr(), libc::O_NONBLOCK | libc::O_RDWR, 0) });
        if *fd == -1 {
            std::mem::forget(fd);
            return Err(Error::LibcError(errno::errno()))
        }
        do_ioctl!(fioclex(*fd)); // non-atomic :( but no O_CLOEXEC yet.

        let mut dev = Device {
            fd: *fd,
            ty: Types::empty(),
            name: unsafe { CString::from_vec_unchecked(Vec::new()) },
            phys: None,
            uniq: None,
            id: unsafe { std::mem::zeroed() },
            props: Props::empty(),
            driver_version: (0, 0, 0),
            key_bits: FixedBitSet::with_capacity(Key::KEY_MAX as usize),
            key_vals: FixedBitSet::with_capacity(Key::KEY_MAX as usize),
            rel: RelativeAxis::empty(),
            abs: AbsoluteAxis::empty(),
            abs_vals: vec![],
            switch: Switch::empty(),
            switch_vals: FixedBitSet::with_capacity(0x10),
            led: Led::empty(),
            led_vals: FixedBitSet::with_capacity(0x10),
            misc: Misc::empty(),
            ff: FixedBitSet::with_capacity(FFEffect::FF_MAX as usize + 1),
            ff_stat: FFStatus::empty(),
            rep: Repeat::empty(),
            snd: Sound::empty(),
            pending_events: Vec::with_capacity(64),
        };

        let mut bits: u32 = 0;
        let mut bits64: u64 = 0;
        let mut vec = Vec::with_capacity(256);

        do_ioctl!(eviocgbit(*fd, 0, 4, &mut bits as *mut _ as *mut u8));
        dev.ty = Types::from_bits(bits).expect("evdev: unexpected type bits! report a bug");

        let dev_len = do_ioctl!(eviocgname(*fd, vec.as_mut_ptr(), 255));
        unsafe { vec.set_len(dev_len as usize - 1) };
        dev.name = CString::new(vec.clone()).unwrap();

        let phys_len = unsafe { ioctl::eviocgphys(*fd, vec.as_mut_ptr(), 255) };
        if phys_len > 0 {
            unsafe { vec.set_len(phys_len as usize - 1) };
            dev.phys = Some(CString::new(vec.clone()).unwrap());
        }

        let uniq_len = unsafe { ioctl::eviocguniq(*fd, vec.as_mut_ptr(), 255) };
        if uniq_len > 0 {
            unsafe { vec.set_len(uniq_len as usize - 1) };
            dev.uniq = Some(CString::new(vec.clone()).unwrap());
        }

        do_ioctl!(eviocgid(*fd, &mut dev.id));
        let mut driver_version: i32 = 0;
        do_ioctl!(eviocgversion(*fd, &mut driver_version));
        dev.driver_version =
            (((driver_version >> 16) & 0xff) as u8,
             ((driver_version >> 8) & 0xff) as u8,
              (driver_version & 0xff) as u8);

        do_ioctl!(eviocgprop(*fd, &mut bits as *mut _ as *mut u8, 0x1f)); // todo: handle old kernel
        dev.props = Props::from_bits(bits).expect("evdev: unexpected prop bits! report a bug");

        if dev.ty.contains(KEY) {
            do_ioctl!(eviocgbit(*fd, ffs(KEY.bits()), dev.key_bits.len() as libc::c_int, dev.key_bits.as_mut_slice().as_mut_ptr() as *mut u8));
        }

        if dev.ty.contains(RELATIVE) {
            do_ioctl!(eviocgbit(*fd, ffs(RELATIVE.bits()), 0xf, &mut bits as *mut _ as *mut u8));
            dev.rel = RelativeAxis::from_bits(bits).expect("evdev: unexpected rel bits! report a bug");
        }

        if dev.ty.contains(ABSOLUTE) {
            do_ioctl!(eviocgbit(*fd, 31 - ABSOLUTE.bits().leading_zeros(), 0x3f, &mut bits64 as *mut _ as *mut u8));
            dev.abs = AbsoluteAxis::from_bits(bits64).expect("evdev: unexpected abs bits! report a bug");
            dev.abs_vals = vec![ioctl::input_absinfo::default(); 0x3f];
        }

        if dev.ty.contains(SWITCH) {
            do_ioctl!(eviocgbit(*fd, ffs(SWITCH.bits()), 0xf, &mut bits as *mut _ as *mut u8));
            dev.switch = Switch::from_bits(bits).expect("evdev: unexpected switch bits! report a bug");
        }

        if dev.ty.contains(LED) {
            do_ioctl!(eviocgbit(*fd, ffs(LED.bits()), 0xf, &mut bits as *mut _ as *mut u8));
            dev.led = Led::from_bits(bits).expect("evdev: unexpected led bits! report a bug");
        }

        if dev.ty.contains(MISC) {
            do_ioctl!(eviocgbit(*fd, ffs(MISC.bits()), 0x7, &mut bits as *mut _ as *mut u8));
            dev.misc = Misc::from_bits(bits).expect("evdev: unexpected misc bits! report a bug");
        }

        //do_ioctl!(eviocgbit(*fd, ffs(FORCEFEEDBACK.bits()), 0x7f, &mut bits as *mut _ as *mut u8));

        if dev.ty.contains(SOUND) {
            do_ioctl!(eviocgbit(*fd, 31 - SOUND.bits().leading_zeros(), 0x7, &mut bits as *mut _ as *mut u8));
            dev.snd = Sound::from_bits(bits).expect("evdev: unexpected sound bits! report a bug");
        }

        try!(dev.sync());

        std::mem::forget(fd);
        Ok(dev)
    }

    /// Synchronize the `Device` state with the kernel device state.
    ///
    /// If there is an error at any point, the state will not be synchronized completely.
    pub fn sync(&mut self) -> Result<(), Error> {
        if self.ty.contains(KEY) {
            do_ioctl!(eviocgkey(self.fd, self.key_vals.as_mut_slice().as_mut_ptr() as *mut _ as *mut u8, self.key_vals.len()));
        }
        if self.ty.contains(ABSOLUTE) {
            for idx in (0..0x28) {
                let abs = 1 << idx;
                // ignore multitouch, we'll handle that later.
                if abs < ABS_MT_SLOT.bits() && self.abs.bits() & abs == 1 {
                    do_ioctl!(eviocgabs(self.fd, idx, &mut self.abs_vals[idx as usize]));
                }
            }
        }
        if self.ty.contains(SWITCH) {
            do_ioctl!(eviocgsw(self.fd, &mut self.switch_vals.as_mut_slice().as_mut_ptr() as *mut _ as *mut u8, self.switch_vals.len()));
        }
        if self.ty.contains(LED) {
            do_ioctl!(eviocgled(self.fd, &mut self.led_vals.as_mut_slice().as_mut_ptr() as *mut _ as *mut u8, self.led_vals.len()));
        }

        Ok(())
    }

    fn fill_events(&mut self) {
        let mut buf = &mut self.pending_events;
        loop {
            buf.reserve(20);
            let pre_len = buf.len();
            let sz = unsafe {
                libc::read(self.fd,
                           buf.as_mut_ptr()
                              .offset(pre_len as isize) as *mut libc::c_void,
                           (size_of::<ioctl::input_event>() * (buf.capacity() - pre_len)) as libc::size_t)
            };
            if sz == -1 {
                let errno = errno::errno();
                if errno != errno::Errno(libc::EAGAIN) {
                    println!("ERROR! evdev needs to figure out how to expose this :( {}", errno);
                } else {
                    break;
                }
            } else {
                unsafe {
                    buf.set_len(pre_len + (sz as usize / size_of::<ioctl::input_event>()));
                }
            }
        }
    }

    /// Exposes the raw evdev events without doing synchronization on SYN_DROPPED.
    pub fn raw_events(&mut self) -> RawEvents {
        self.fill_events();
        RawEvents::new(self)
    }


    pub fn events(&mut self) -> Events {
        Events(self)
    }
}

pub struct Events<'a>(&'a mut Device);

#[cfg(feature = "unstable")]
pub struct RawEvents<'a>(std::vec::Drain<'a, ioctl::input_event>);

#[cfg(not(feature = "unstable"))]
pub struct RawEvents<'a>(&'a mut Device);

#[cfg(feature = "unstable")]
impl<'a> RawEvents<'a> {
    fn new(dev: &'a mut Device) -> RawEvents<'a> {
        RawEvents(dev.pending_events.drain(..))
    }
}

#[cfg(not(feature = "unstable"))]
impl<'a> RawEvents<'a> {
    fn new(dev: &'a mut Device) -> RawEvents<'a> {
        dev.pending_events.reverse();
        RawEvents(dev)
    }
}

#[cfg(not(feature = "unstable"))]
impl<'a> Drop for RawEvents<'a> {
    fn drop(&mut self) {
        self.0.pending_events.reverse();
    }
}

#[cfg(feature = "unstable")]
impl<'a> Iterator for RawEvents<'a> {
    type Item = ioctl::input_event;

    #[inline(always)]
    fn next(&mut self) -> Option<ioctl::input_event> {
        self.0.next()
    }
}

#[cfg(not(feature = "unstable"))]
impl<'a> Iterator for RawEvents<'a> {
    type Item = ioctl::input_event;

    #[inline(always)]
    fn next(&mut self) -> Option<ioctl::input_event> {
        self.0.pending_events.pop()
    }
}

/// Crawls `/dev/input` for evdev devices.
///
/// Will not bubble up any errors in opening devices or traversing the directory. Instead returns
/// an empty vector or omits the devices that could not be opened.
pub fn enumerate() -> Vec<Device> {
    let mut res = Vec::new();
    if let Ok(dir) = std::fs::read_dir("/dev/input") {
        for entry in dir {
            if let Ok(entry) = entry {
                if let Ok(dev) = Device::open(&entry.path()) {
                    res.push(dev)
                }
            }
        }
    }
    res
}
