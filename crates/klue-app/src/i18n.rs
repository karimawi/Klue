//! English and Arabic strings.

use chrono::{DateTime, Datelike, Local, Timelike, Utc};
use klue_core::{
    BundleKind, BundleState, Carrier, FetchError, LineType, Unit, format,
    store::{Clock, Lang},
};

use crate::product::Product;

pub struct Strings {
    pub app: &'static str,
    pub light: &'static str,
    pub dark: &'static str,
    pub match_long: &'static str,
    pub add: &'static str,
    pub settings: &'static str,
    pub add_number: &'static str,
    pub mobile: &'static str,
    pub landline: &'static str,
    /// A home phone line (WE Ardy).
    pub voice: &'static str,
    pub registered_to: &'static str,
    pub refresh: &'static str,
    pub rename: &'static str,
    pub alerts: &'static str,
    pub package: &'static str,
    pub below_alert: &'static str,
    pub remaining: &'static str,
    pub change: &'static str,
    pub next_renewal: &'static str,
    pub cycle: &'static str,
    pub subscribed: &'static str,
    pub renews: &'static str,
    pub bundles: &'static str,
    pub col_bundle: &'static str,
    pub col_rem: &'static str,
    pub col_total: &'static str,
    pub col_left: &'static str,
    pub col_alert: &'static str,
    pub off: &'static str,
    pub empty_body: &'static str,
    pub steps: [&'static str; 3],
    pub new_number: &'static str,
    pub password: &'static str,
    pub owner_name: &'static str,
    pub from_account: &'static str,
    pub nick_optional: &'static str,
    pub nick_ph: &'static str,
    pub verifying: &'static str,
    pub cancel: &'static str,
    pub try_again: &'static str,
    pub err_title: &'static str,
    pub rename_title: &'static str,
    pub nickname: &'static str,
    pub nick_hint: &'static str,
    pub shown_as: &'static str,
    pub use_owner: &'static str,
    pub save: &'static str,
    pub quota_alerts: &'static str,
    pub renewal_reminder: &'static str,
    pub remind_me: &'static str,
    pub before_renewal: &'static str,
    pub days: &'static str,
    pub hours: &'static str,
    pub renew_day: &'static str,
    pub preview: &'static str,
    pub theme: &'static str,
    pub match_hint: &'static str,
    pub language: &'static str,
    pub check_every: &'static str,
    /// Under "Check quotas every": numbers can have their own.
    pub check_every_hint: &'static str,
    /// Edit dialog: this number's own refresh interval.
    pub check_number_every: &'static str,
    pub follow_settings: &'static str,
    pub check_number_hint: &'static str,
    pub time_format: &'static str,
    pub startup: &'static str,
    pub tray: &'static str,
    pub disclaimer: &'static str,
    pub done: &'static str,
    pub open_app: &'static str,
    pub settings_title: &'static str,
    pub empty_kicker: &'static str,
    pub coming_soon_body: &'static str,
    pub not_synced: &'static str,
    pub syncing: &'static str,
    pub synced_now: &'static str,
    pub waiting_first_sync: &'static str,
    pub owner_unknown: &'static str,
    pub remove: &'static str,
    pub remove_hint: &'static str,
    pub remove_confirm: &'static str,
    pub captcha_title: &'static str,
    pub captcha_body: &'static str,
    pub captcha_ph: &'static str,
    pub continue_: &'static str,
    pub already_added: &'static str,
    pub already_archived: &'static str,
    /// Archiving: the Edit dialog, and the Archived button and dialog.
    pub archive: &'static str,
    pub archive_hint: &'static str,
    pub archived_title: &'static str,
    pub archived_hint: &'static str,
    pub archived_rail: &'static str,
    /// Beside one archived number in a list.
    pub archived_note: &'static str,
    pub restore: &'static str,
    pub no_password: &'static str,
    pub quick_view: &'static str,
    pub refresh_all: &'static str,
    pub quit: &'static str,
    pub tray_empty: &'static str,
    pub balance: &'static str,
    pub line_info: &'static str,
    pub plan: &'static str,
    pub payment_type: &'static str,
    pub prepaid: &'static str,
    pub postpaid: &'static str,
    pub status: &'static str,
    pub since: &'static str,
    pub payments: &'static str,
    pub no_payments: &'static str,
    pub payments_unavailable: &'static str,
    pub col_date: &'static str,
    pub col_method: &'static str,
    pub col_amount: &'static str,
    pub col_balance_after: &'static str,
    pub make_main: &'static str,
    pub main_default: &'static str,
    pub main_picked: &'static str,
    pub egp: &'static str,
    /// Bundle states (see [`state_name`]) and the note under an owed amount.
    pub borrowed: &'static str,
    pub frozen: &'static str,
    pub inactive: &'static str,
    /// Line card: when a prepaid line expires unless recharged.
    pub valid_until: &'static str,
    pub owed_note: &'static str,
    /// The Add wizard: its steps, their titles, and the sign-in fields.
    pub wizard_steps: [&'static str; 4],
    pub pick_carrier: &'static str,
    pub pick_line: &'static str,
    pub ready_to_add: &'static str,
    pub soon: &'static str,
    pub coming_soon: &'static str,
    pub back: &'static str,
    pub sign_in: &'static str,
    pub signed_in: &'static str,
    pub mobile_number: &'static str,
    pub landline_number: &'static str,
    pub area_code_hint: &'static str,
    pub we_mobile_hint: &'static str,
    /// Finish step: the account's other lines (Orange).
    pub looking_for_lines: &'static str,
    pub add_both: &'static str,
    /// Export and import.
    pub your_numbers: &'static str,
    pub export_number: &'static str,
    pub export_hint: &'static str,
    pub export_numbers: &'static str,
    pub export_numbers_hint: &'static str,
    pub select_all: &'static str,
    pub select_none: &'static str,
    pub export_button: &'static str,
    pub import_button: &'static str,
    /// The home page and its groups.
    pub home: &'static str,
    pub my_lines: &'static str,
    pub group: &'static str,
    pub new_group: &'static str,
    pub edit_group: &'static str,
    pub group_name: &'static str,
    pub group_ph: &'static str,
    pub create: &'static str,
    pub delete_group: &'static str,
    pub delete_group_hint: &'static str,
    pub add_lines: &'static str,
    pub add_lines_hint: &'static str,
    pub group_lines_title: &'static str,
    pub group_lines_hint: &'static str,
    pub group_empty: &'static str,
    pub no_lines_yet: &'static str,
    pub other_bundles: &'static str,
    pub bundle_gone: &'static str,
    pub import_numbers: &'static str,
    pub import_hint: &'static str,
    pub import_file: &'static str,
    pub export_contents: &'static str,
    pub with_passwords: &'static str,
    pub with_passwords_hint: &'static str,
    pub passphrase: &'static str,
    pub passphrase_hint: &'static str,
    pub import_passphrase_hint: &'static str,
    pub save_file: &'static str,
    pub saving: &'static str,
    pub importing: &'static str,
    pub devices_title: &'static str,
    pub devices_body: &'static str,
    pub line_new: &'static str,
    pub line_here: &'static str,
    pub line_no_password: &'static str,
    pub line_unsupported: &'static str,
    pub choose_file: &'static str,
    pub import: &'static str,
    /// The package's line speed (WE).
    pub speed: &'static str,
    pub after_quota: &'static str,
    /// Quota card bundle navigation.
    pub main_quota: &'static str,
    pub prev_bundle: &'static str,
    pub next_bundle: &'static str,
    /// Settings: alerts for all numbers.
    pub all_alerts: &'static str,
    pub all_alerts_hint: &'static str,
    pub all_low_quota: &'static str,
    pub percent_left: &'static str,
    pub all_other_bundles: &'static str,
    pub apply_all: &'static str,
    pub applied_all: &'static str,
    /// Settings: About.
    pub about: &'static str,
    pub made_by: &'static str,
    pub repo_link: &'static str,
    pub website_link: &'static str,
    pub check_updates: &'static str,
    pub checking_updates: &'static str,
    pub up_to_date: &'static str,
    pub update_check_failed: &'static str,
    pub view_update: &'static str,
    /// The update dialog.
    pub update_kicker: &'static str,
    pub whats_new: &'static str,
    pub update_install: &'static str,
    pub update_download: &'static str,
    pub update_later: &'static str,
    pub update_skip: &'static str,
    pub update_downloading: &'static str,
    pub update_installing: &'static str,
    pub update_opened: &'static str,
    pub update_failed: &'static str,
    pub release_page: &'static str,
    pub update_notify_body: &'static str,
    /// The disclaimer shown on first launch and after each update.
    pub disc_kicker: &'static str,
    pub disc_title: &'static str,
    pub disc_passwords_title: &'static str,
    pub disc_passwords: &'static str,
    pub disc_unofficial_title: &'static str,
    pub disc_unofficial: &'static str,
    pub disc_risk_title: &'static str,
    pub disc_risk: &'static str,
    pub disc_accept: &'static str,
    /// Title bar button for the debug console.
    pub debug_console: &'static str,
    /// Title of the usage graph section.
    pub usage: &'static str,
    /// The usage graph's views.
    pub usage_days: &'static str,
    pub usage_hours: &'static str,
    /// Why a view isn't available yet.
    pub usage_days_later: &'static str,
    pub usage_hours_later: &'static str,
    /// Captions of the graph's two panels.
    pub chart_left: &'static str,
    pub chart_per_day: &'static str,
    pub chart_per_hour: &'static str,
    /// Figures beside the graph.
    pub usage_today: &'static str,
    pub so_far: &'static str,
    pub no_refresh_today: &'static str,
    pub per_day: &'static str,
    pub per_hour: &'static str,
    pub avg_cycle: &'static str,
    pub avg_refreshes: &'static str,
    pub at_this_rate: &'static str,
    pub lasts_renewal: &'static str,
    pub no_usage_yet: &'static str,
    /// When there's too little history to draw.
    pub not_enough_title: &'static str,
    pub not_enough_body: &'static str,
    pub not_enough_cycle: &'static str,
    pub not_enough_samples: &'static str,
    /// Notes under the graph.
    pub chart_dots: &'static str,
    pub chart_gaps: &'static str,
}

pub static EN: Strings = Strings {
    app: "Klue", light: "Light", dark: "Dark", match_long: "Match carrier", add: "Add", settings: "Settings",
    add_number: "Add number", mobile: "Mobile", landline: "DSL", voice: "Landline", registered_to: "Registered to", refresh: "Refresh",
    rename: "Edit", alerts: "Alerts", package: "Package", below_alert: "Below alert", remaining: "remaining", change: "Change",
    next_renewal: "Next renewal", cycle: "Billing cycle", subscribed: "Subscribed", renews: "Renews", bundles: "Bundles & add-ons",
    col_bundle: "Bundle", col_rem: "Remaining", col_total: "Total", col_left: "Left", col_alert: "Alert", off: "Off",
    empty_body: "Add a number with the password of its carrier’s app. Klue signs in, reads the quota, and fills in the registered owner for you. Already using Klue on another computer? Import its numbers from a file.",
    steps: ["Pick the carrier and the kind of line", "Sign in the way its app does", "The quota and the owner’s name load by themselves"],
    new_number: "New number",
    password: "Account password", owner_name: "Owner name", from_account: "From the account",
    nick_optional: "Nickname (optional)",
    nick_ph: "e.g. Home internet", verifying: "Verifying…", cancel: "Cancel",
    try_again: "Try again", err_title: "Password not accepted", rename_title: "Edit number", nickname: "Nickname",
    nick_hint: "Only changes how the number appears in Klue.", shown_as: "Shown as", use_owner: "Use owner name", save: "Save",
    quota_alerts: "Quota alerts", renewal_reminder: "Renewal reminder", remind_me: "Remind me",
    before_renewal: "before renewal", days: "Days", hours: "Hours", renew_day: "Also notify on renewal day", preview: "Preview",
    theme: "Theme", match_hint: "Match carrier recolors the app to each carrier’s colors as you switch tabs.", language: "Language",
    check_every: "Check quotas every", check_every_hint: "Numbers can have their own, from their Edit dialog.", check_number_every: "Check this number every", follow_settings: "As in Settings", check_number_hint: "Klue checks the number in the background this often. Refresh checks it now.", time_format: "Time format", startup: "Launch at startup", tray: "Keep running in the tray",
    disclaimer: "Unofficial app. Not affiliated with any carrier.", done: "Done", open_app: "Open Klue", settings_title: "Settings",
    empty_kicker: "No numbers",
    coming_soon_body: "Klue can’t sign in to this carrier yet. WE home internet, Vodafone mobile and Orange lines work today; e& is next.",
    not_synced: "Not synced yet", syncing: "Syncing…", synced_now: "Synced just now", waiting_first_sync: "Waiting for the first sync…", owner_unknown: "Not provided by the carrier", remove: "Remove number",
    remove_hint: "Removes the number and its saved password from this computer.", remove_confirm: "Click again to remove",
    captcha_title: "Type the characters", captcha_body: "WE wants to check you’re human before signing in. Type the characters in the picture.",
    captcha_ph: "Characters", continue_: "Continue", already_added: "This number is already in Klue.", already_archived: "This number is archived in Klue. Restore it from Archived.", archive: "Archive number", archive_hint: "Stops checking it and hides it. The password stays saved, and Archived in the sidebar brings it back.", archived_title: "Archived numbers", archived_hint: "Klue doesn’t check these. Restore one to see it and check it again.", archived_rail: "Archived", archived_note: "Archived", restore: "Restore",
    no_password: "The saved password is missing. Remove the number and add it again.",
    quick_view: "Quick view", refresh_all: "Refresh all", quit: "Quit Klue",
    tray_empty: "No numbers yet. Open Klue to add one.",
    balance: "Balance",
    line_info: "Line",
    plan: "Plan",
    payment_type: "Type",
    prepaid: "Prepaid",
    postpaid: "Postpaid",
    status: "Status",
    since: "Since",
    payments: "Payments",
    no_payments: "No payments in the last 90 days.",
    payments_unavailable: "Payment history isn't available for this line yet.",
    col_date: "Date",
    col_method: "Method",
    col_amount: "Amount",
    col_balance_after: "Balance after",
    make_main: "Make main",
    main_default: "Use the default",
    main_picked: "Picked by you",
    egp: "EGP",
    borrowed: "Borrowed",
    frozen: "Frozen",
    inactive: "Not active",
    valid_until: "Valid until",
    owed_note: "Your next recharge pays it first.",
    wizard_steps: ["Carrier", "Line", "Sign in", "Finish"],
    pick_carrier: "Which carrier is it?",
    pick_line: "What kind of line?",
    ready_to_add: "Ready to add",
    soon: "Soon",
    coming_soon: "Coming soon",
    back: "Back",
    sign_in: "Sign in",
    signed_in: "Signed in",
    mobile_number: "Mobile number",
    landline_number: "Landline number",
    area_code_hint: "With the area code, the way you sign in to My WE.",
    we_mobile_hint: "WE Mobile lines are read through My WE: sign in with your home internet’s landline number, and Klue finds the mobile lines on the account.",
    looking_for_lines: "Looking for other lines on this account…",
    add_both: "Add both numbers",
    your_numbers: "Your numbers",
    export_number: "Export number",
    export_hint: "Saves it to a file you can import on another computer.",
    export_numbers: "Export numbers",
    export_numbers_hint: "Several numbers, or all of them, in one file, to move them to another computer.",
    select_all: "Select all",
    select_none: "Select none",
    export_button: "Export…",
    import_button: "Import…",
    home: "Home",
    my_lines: "My lines",
    group: "Group",
    new_group: "New group",
    edit_group: "Edit group",
    group_name: "Group name",
    group_ph: "e.g. Family",
    create: "Create",
    delete_group: "Delete group",
    delete_group_hint: "Removes this tab from Home. Its lines stay in Klue.",
    add_lines: "Add lines",
    add_lines_hint: "Pick the lines to see here",
    group_lines_title: "Lines on this tab",
    group_lines_hint: "Switch on the lines to glance at here, from any carrier. A line shows its main bundle; switch on its other bundles to give each a card too.",
    group_empty: "Nothing here yet. Add the lines you want to see at a glance, from any carrier.",
    no_lines_yet: "Add a number first, then put it here.",
    other_bundles: "Other bundles",
    bundle_gone: "Not on this line right now",
    import_numbers: "Import numbers",
    import_hint: "From a file Klue exported on another computer.",
    import_file: "Import from a file",
    export_contents: "The file holds each number’s settings, its latest reading and its usage history.",
    with_passwords: "Include the passwords",
    with_passwords_hint: "Sealed with a passphrase you choose. Without them, a number has to be added on the other computer before its file is imported.",
    passphrase: "Passphrase",
    passphrase_hint: "At least 8 characters. You’ll type it when you import the file.",
    import_passphrase_hint: "The one chosen when the file was exported.",
    save_file: "Save file…",
    saving: "Saving…",
    importing: "Importing…",
    devices_title: "Each computer signs in on its own",
    devices_body: "Klue doesn’t put your data online, so every computer with a number signs in to the carrier and reads it by itself. The more computers check the same account, the more sign-ins the carrier sees, and the likelier it is to slow them down or block them for a while.",
    line_new: "New on this computer",
    line_here: "Already here: gets the file’s history",
    line_no_password: "No password in the file: add it first, then import",
    line_unsupported: "This version of Klue can’t read it",
    choose_file: "Choose another file",
    import: "Import",
    speed: "Speed",
    after_quota: "After the quota",
    main_quota: "The main quota",
    prev_bundle: "Previous bundle",
    next_bundle: "Next bundle",
    all_alerts: "Alerts for all numbers",
    all_alerts_hint: "New numbers start with these. Apply them to give every number these settings now.",
    all_low_quota: "Alert on each main quota at",
    percent_left: "% left",
    all_other_bundles: "Also alert on the other bundles",
    apply_all: "Apply to all numbers",
    applied_all: "Applied to every number.",
    about: "About Klue",
    made_by: "Made by",
    repo_link: "GitHub repository",
    website_link: "Website",
    check_updates: "Check for updates",
    checking_updates: "Checking for updates…",
    up_to_date: "Klue is up to date.",
    update_check_failed: "Couldn't check for updates.",
    view_update: "View the update",
    update_kicker: "Update",
    whats_new: "What's new",
    update_install: "Download and install",
    update_download: "Download",
    update_later: "Later",
    update_skip: "Skip this version",
    update_downloading: "Downloading…",
    update_installing: "Installing… Klue will restart by itself.",
    update_opened: "The download is open. Finish installing it there, then start Klue again.",
    update_failed: "The update couldn't be installed.",
    release_page: "Open the release page",
    update_notify_body: "Open Klue to install it.",
    disc_kicker: "Before you start",
    disc_title: "Please read and accept",
    disc_passwords_title: "Your passwords stay on this device",
    disc_passwords: "Klue keeps them in your system's credential store (Windows Credential Manager, the macOS Keychain, or the Secret Service on Linux) and sends them nowhere but to your carrier, when it signs in for you.",
    disc_unofficial_title: "Klue isn't an official app",
    disc_unofficial: "It talks to your carrier the way the carrier's own app and website do, through interfaces worked out from them (reverse engineered). Carriers don't seem to look for tools like this today, but they may make them harder to use in the future, or even warn or penalise accounts that use them.",
    disc_risk_title: "Use it at your own risk",
    disc_risk: "The developers aren't responsible for anything your carrier does about accounts that use Klue.",
    disc_accept: "I understand and accept",
    debug_console: "Debug console",
    usage: "Usage",
    usage_days: "Days",
    usage_hours: "Hours",
    usage_days_later: "Shows once a day of refreshes is saved",
    usage_hours_later: "Shows after a few hours of refreshes about every hour",
    chart_left: "What was left",
    chart_per_day: "Used per day",
    chart_per_hour: "Used per hour",
    usage_today: "Today",
    so_far: "So far",
    no_refresh_today: "No refresh yet today",
    per_day: "Per day",
    per_hour: "Per hour",
    avg_cycle: "This cycle’s average",
    avg_refreshes: "From the refreshes so far",
    at_this_rate: "At this rate",
    lasts_renewal: "Lasts until it renews",
    no_usage_yet: "Nothing used yet",
    not_enough_title: "Not enough refreshes for a graph yet",
    not_enough_body: "Klue saves what’s left at every refresh and draws the graph after a few hours of them.",
    not_enough_cycle: "Until then, the figures below are estimates from what this cycle has used so far.",
    not_enough_samples: "Until then, the figures below are estimates from the refreshes so far.",
    chart_dots: "Each dot is a refresh. Hover over the graph for the exact time and amounts.",
    chart_gaps: "Dashed lines and fainter bars span long waits between refreshes; their usage is spread out evenly.",
};

pub static AR: Strings = Strings {
    app: "Klue", light: "فاتح", dark: "داكن", match_long: "ألوان الشركة", add: "إضافة", settings: "الإعدادات",
    add_number: "إضافة رقم", mobile: "محمول", landline: "DSL", voice: "أرضي", registered_to: "مسجّل باسم", refresh: "تحديث", rename: "تعديل",
    alerts: "التنبيهات", package: "الباقة", below_alert: "أقل من حد التنبيه", remaining: "متبقي", change: "تعديل",
    next_renewal: "التجديد القادم", cycle: "دورة الاشتراك", subscribed: "تاريخ الاشتراك", renews: "يتجدد", bundles: "الباقات والإضافات",
    col_bundle: "الباقة", col_rem: "المتبقي", col_total: "الإجمالي", col_left: "النسبة", col_alert: "التنبيه", off: "غير مفعّل",
    empty_body: "أضف الرقم بكلمة مرور تطبيق شركته. يسجّل Klue الدخول ويقرأ الباقة ويملأ اسم صاحب الخط تلقائياً. تستخدم Klue على جهاز آخر؟ استورد أرقامه من ملف.",
    steps: ["اختر الشركة ونوع الخط", "سجّل الدخول كما في تطبيقها", "تظهر الباقة واسم صاحب الخط تلقائياً"],
    new_number: "رقم جديد",
    password: "كلمة مرور الحساب", owner_name: "اسم صاحب الخط", from_account: "من بيانات الحساب",
    nick_optional: "اسم مختصر (اختياري)",
    nick_ph: "مثال: إنترنت البيت", verifying: "جارٍ التحقق…", cancel: "إلغاء",
    try_again: "إعادة المحاولة", err_title: "كلمة المرور غير صحيحة",
    rename_title: "تعديل الرقم", nickname: "الاسم المختصر", nick_hint: "يغيّر طريقة ظهور الرقم في Klue فقط.",
    shown_as: "يظهر باسم", use_owner: "استخدام اسم صاحب الخط", save: "حفظ",
    quota_alerts: "تنبيهات الرصيد", renewal_reminder: "تذكير التجديد", remind_me: "ذكّرني قبل", before_renewal: "من التجديد",
    days: "أيام", hours: "ساعات", renew_day: "تنبيه يوم التجديد أيضاً", preview: "معاينة", theme: "المظهر",
    match_hint: "ألوان الشركة: يتغيّر لون التطبيق حسب الشركة عند التنقل بين التبويبات.", language: "اللغة",
    check_every: "تحديث الرصيد كل", check_every_hint: "يمكن أن يكون لكل رقم توقيته الخاص من نافذة تعديله.", check_number_every: "تحديث هذا الرقم كل", follow_settings: "كما في الإعدادات", check_number_hint: "يحدّث Klue الرقم في الخلفية بهذا التوقيت، وزر التحديث يحدّثه الآن.", time_format: "صيغة الوقت", startup: "التشغيل مع بدء النظام", tray: "العمل في الخلفية",
    disclaimer: "تطبيق غير رسمي وغير تابع لأي شركة اتصالات.", done: "تم", open_app: "فتح Klue", settings_title: "الإعدادات",
    empty_kicker: "لا توجد أرقام",
    coming_soon_body: "لا يستطيع Klue تسجيل الدخول إلى هذه الشركة بعد. يعمل حالياً إنترنت وي المنزلي وخطوط فودافون المحمول وخطوط أورنج، وإي آند في الطريق.",
    not_synced: "لم يتم التحديث بعد", syncing: "جارٍ التحديث…", synced_now: "تم التحديث الآن", waiting_first_sync: "في انتظار أول تحديث…", owner_unknown: "لم توفره الشركة", remove: "حذف الرقم",
    remove_hint: "يحذف الرقم وكلمة المرور المحفوظة من هذا الجهاز.", remove_confirm: "اضغط مرة أخرى للحذف",
    captcha_title: "اكتب الحروف", captcha_body: "تريد وي التأكد من أنك لست برنامجاً قبل تسجيل الدخول. اكتب الحروف الظاهرة في الصورة.",
    captcha_ph: "الحروف", continue_: "متابعة", already_added: "هذا الرقم مضاف بالفعل في Klue.", already_archived: "هذا الرقم مؤرشف في Klue. استعده من «المؤرشفة».", archive: "أرشفة الرقم", archive_hint: "يوقف تحديثه ويخفيه، وتبقى كلمة المرور محفوظة ويمكن استعادته من «المؤرشفة» في الشريط الجانبي.", archived_title: "الأرقام المؤرشفة", archived_hint: "لا يحدّث Klue هذه الأرقام. استعد رقماً لتراه ويُحدَّث مرة أخرى.", archived_rail: "المؤرشفة", archived_note: "مؤرشف", restore: "استعادة",
    no_password: "كلمة المرور المحفوظة غير موجودة. احذف الرقم ثم أضفه مرة أخرى.",
    quick_view: "عرض سريع", refresh_all: "تحديث الكل", quit: "إغلاق Klue",
    tray_empty: "لا توجد أرقام بعد. افتح Klue لإضافة رقم.",
    balance: "الرصيد",
    line_info: "الخط",
    plan: "النظام",
    payment_type: "النوع",
    prepaid: "مسبق الدفع",
    postpaid: "فاتورة",
    status: "الحالة",
    since: "منذ",
    payments: "المدفوعات",
    no_payments: "لا توجد مدفوعات في آخر ٩٠ يومًا.",
    payments_unavailable: "سجل المدفوعات غير متاح لهذا الخط بعد.",
    col_date: "التاريخ",
    col_method: "الطريقة",
    col_amount: "المبلغ",
    col_balance_after: "الرصيد بعدها",
    make_main: "اجعلها الرئيسية",
    main_default: "الافتراضي",
    main_picked: "اخترتها أنت",
    egp: "جنيه",
    borrowed: "مستلفة",
    frozen: "موقوفة",
    inactive: "غير مفعّلة",
    valid_until: "صالح حتى",
    owed_note: "يُخصم أولًا من الشحنة القادمة.",
    wizard_steps: ["الشركة", "نوع الخط", "تسجيل الدخول", "الإنهاء"],
    pick_carrier: "ما شركة الخط؟",
    pick_line: "ما نوع الخط؟",
    ready_to_add: "جاهز للإضافة",
    soon: "قريباً",
    coming_soon: "قريباً",
    back: "رجوع",
    sign_in: "تسجيل الدخول",
    signed_in: "تم تسجيل الدخول",
    mobile_number: "رقم المحمول",
    landline_number: "رقم الخط الأرضي",
    area_code_hint: "مع كود المحافظة، كما تسجّل الدخول في My WE.",
    we_mobile_hint: "يقرأ Klue خطوط وي موبايل من خلال My WE: سجّل الدخول برقم الخط الأرضي للإنترنت المنزلي، وسيجد Klue خطوط المحمول على الحساب.",
    looking_for_lines: "جارٍ البحث عن خطوط أخرى على هذا الحساب…",
    add_both: "إضافة الرقمين",
    your_numbers: "أرقامك",
    export_number: "تصدير الرقم",
    export_hint: "يحفظه في ملف يمكنك استيراده على جهاز آخر.",
    export_numbers: "تصدير الأرقام",
    export_numbers_hint: "عدة أرقام أو كلها في ملف واحد، لنقلها إلى جهاز آخر.",
    select_all: "تحديد الكل",
    select_none: "إلغاء التحديد",
    export_button: "تصدير…",
    import_button: "استيراد…",
    home: "الرئيسية",
    my_lines: "خطوطي",
    group: "مجموعة",
    new_group: "مجموعة جديدة",
    edit_group: "تعديل المجموعة",
    group_name: "اسم المجموعة",
    group_ph: "مثال: العائلة",
    create: "إنشاء",
    delete_group: "حذف المجموعة",
    delete_group_hint: "يحذف هذا التبويب من الرئيسية، وتبقى خطوطه في Klue.",
    add_lines: "إضافة خطوط",
    add_lines_hint: "اختر الخطوط التي تراها هنا",
    group_lines_title: "الخطوط في هذا التبويب",
    group_lines_hint: "فعّل الخطوط التي تريد رؤيتها هنا من أي شركة. يظهر كل خط بباقته الرئيسية، وفعّل باقاته الأخرى لتظهر كل منها في بطاقة أيضاً.",
    group_empty: "لا شيء هنا بعد. أضف الخطوط التي تريد رؤيتها في لمحة، من أي شركة.",
    no_lines_yet: "أضف رقماً أولاً، ثم ضعه هنا.",
    other_bundles: "باقات أخرى",
    bundle_gone: "غير موجودة على الخط حالياً",
    import_numbers: "استيراد أرقام",
    import_hint: "من ملف صدّره Klue على جهاز آخر.",
    import_file: "استيراد من ملف",
    export_contents: "يحفظ الملف إعدادات كل رقم وآخر قراءة له وسجل استخدامه.",
    with_passwords: "تضمين كلمات المرور",
    with_passwords_hint: "تُحمى بعبارة مرور تختارها. بدونها، يجب إضافة الرقم على الجهاز الآخر قبل استيراد ملفه.",
    passphrase: "عبارة المرور",
    passphrase_hint: "8 أحرف على الأقل. ستكتبها عند استيراد الملف.",
    import_passphrase_hint: "العبارة التي اختيرت عند تصدير الملف.",
    save_file: "حفظ الملف…",
    saving: "جارٍ الحفظ…",
    importing: "جارٍ الاستيراد…",
    devices_title: "كل جهاز يسجّل الدخول بنفسه",
    devices_body: "لا يرفع Klue بياناتك إلى الإنترنت، لذا يسجّل كل جهاز عليه الرقم الدخول إلى الشركة ويقرأ بياناته بنفسه. كلما زادت الأجهزة التي تتابع الحساب نفسه زادت مرات تسجيل الدخول، وزادت فرصة أن تبطئها الشركة أو توقفها مؤقتاً.",
    line_new: "جديد على هذا الجهاز",
    line_here: "موجود بالفعل: يُضاف إليه سجل الملف",
    line_no_password: "لا توجد كلمة مرور في الملف: أضفه أولاً ثم استورد الملف",
    line_unsupported: "هذا الإصدار من Klue لا يقرؤه",
    choose_file: "اختيار ملف آخر",
    import: "استيراد",
    speed: "السرعة",
    after_quota: "بعد انتهاء الباقة",
    main_quota: "الباقة الرئيسية",
    prev_bundle: "الباقة السابقة",
    next_bundle: "الباقة التالية",
    all_alerts: "التنبيهات لكل الأرقام",
    all_alerts_hint: "الأرقام الجديدة تبدأ بهذه الإعدادات. طبّقها لتصبح إعدادات كل الأرقام الآن.",
    all_low_quota: "تنبيه للباقة الرئيسية عند",
    percent_left: "% متبقية",
    all_other_bundles: "وتنبيه للباقات الأخرى أيضاً",
    apply_all: "تطبيق على كل الأرقام",
    applied_all: "تم التطبيق على كل الأرقام.",
    about: "عن Klue",
    made_by: "من تطوير",
    repo_link: "المستودع على GitHub",
    website_link: "الموقع",
    check_updates: "التحقق من التحديثات",
    checking_updates: "جارٍ التحقق من التحديثات…",
    up_to_date: "Klue محدَّث بآخر إصدار.",
    update_check_failed: "تعذّر التحقق من التحديثات.",
    view_update: "عرض التحديث",
    update_kicker: "تحديث",
    whats_new: "الجديد",
    update_install: "تنزيل وتثبيت",
    update_download: "تنزيل",
    update_later: "لاحقاً",
    update_skip: "تخطي هذا الإصدار",
    update_downloading: "جارٍ التنزيل…",
    update_installing: "جارٍ التثبيت… سيُعاد تشغيل Klue تلقائياً.",
    update_opened: "تم فتح الملف الذي تم تنزيله. أكمل التثبيت منه ثم شغّل Klue من جديد.",
    update_failed: "تعذّر تثبيت التحديث.",
    release_page: "فتح صفحة الإصدار",
    update_notify_body: "افتح Klue لتثبيته.",
    disc_kicker: "قبل أن تبدأ",
    disc_title: "يُرجى القراءة والموافقة",
    disc_passwords_title: "تبقى كلمات المرور على هذا الجهاز",
    disc_passwords: "يحفظها Klue في مخزن بيانات الاعتماد في نظامك (مدير بيانات الاعتماد في ويندوز، أو سلسلة المفاتيح في macOS، أو خدمة الأسرار في لينكس)، ولا يرسلها إلى أي جهة سوى شركة الاتصالات الخاصة بك عند تسجيل الدخول نيابةً عنك.",
    disc_unofficial_title: "Klue ليس تطبيقاً رسمياً",
    disc_unofficial: "يتواصل مع شركتك بالطريقة التي يتواصل بها تطبيقها وموقعها، عبر واجهات تم التوصل إليها بالهندسة العكسية. لا يبدو أن الشركات تتعقب مثل هذه الأدوات حالياً، لكنها قد تجعل استخدامها أصعب مستقبلاً، بل قد تُنذر الحسابات التي تستخدمها أو تعاقبها.",
    disc_risk_title: "استخدمه على مسؤوليتك الخاصة",
    disc_risk: "لا يتحمل المطورون أي مسؤولية عما قد تتخذه شركتك من إجراءات تجاه الحسابات التي تستخدم Klue.",
    disc_accept: "أفهم ذلك وأوافق",
    debug_console: "وحدة تصحيح الأخطاء",
    usage: "الاستهلاك",
    usage_days: "الأيام",
    usage_hours: "الساعات",
    usage_days_later: "يظهر بعد حفظ تحديثات يوم كامل",
    usage_hours_later: "يظهر بعد بضع ساعات من التحديث كل ساعة تقريباً",
    chart_left: "المتبقي",
    chart_per_day: "الاستخدام في اليوم",
    chart_per_hour: "الاستخدام في الساعة",
    usage_today: "اليوم",
    so_far: "حتى الآن",
    no_refresh_today: "لا يوجد تحديث اليوم بعد",
    per_day: "في اليوم",
    per_hour: "في الساعة",
    avg_cycle: "متوسط هذه الدورة",
    avg_refreshes: "من التحديثات حتى الآن",
    at_this_rate: "بهذا المعدل",
    lasts_renewal: "يكفي حتى التجديد",
    no_usage_yet: "لم يُستخدم شيء بعد",
    not_enough_title: "التحديثات غير كافية لرسم بياني بعد",
    not_enough_body: "يحفظ Klue المتبقي عند كل تحديث، ويرسم البياني بعد بضع ساعات منها.",
    not_enough_cycle: "حتى ذلك الحين، الأرقام أدناه تقديرية مما استُخدم في هذه الدورة حتى الآن.",
    not_enough_samples: "حتى ذلك الحين، الأرقام أدناه تقديرية من التحديثات حتى الآن.",
    chart_dots: "كل نقطة تحديث. مرّر المؤشر فوق الرسم لمعرفة الوقت والكميات بالضبط.",
    chart_gaps: "الخطوط المتقطعة والأعمدة الباهتة تغطي فترات انتظار طويلة بين التحديثات، واستخدامها موزّع بالتساوي.",
};

pub fn strings(lang: Lang) -> &'static Strings {
    match lang {
        Lang::En => &EN,
        Lang::Ar => &AR,
    }
}

/// "Klue 0.3.0 is available".
pub fn update_available(version: &str, lang: Lang) -> String {
    if is_ar(lang) { format!("الإصدار {version} من Klue متاح") } else { format!("Klue {version} is available") }
}

/// "Version 0.2.0" / "You have 0.2.0".
pub fn version_label(version: &str, lang: Lang) -> String {
    if is_ar(lang) { format!("الإصدار {version}") } else { format!("Version {version}") }
}

pub fn you_have(version: &str, lang: Lang) -> String {
    if is_ar(lang) { format!("لديك الإصدار {version}.") } else { format!("You have {version}.") }
}

/// "4.2 of 12.8 MB".
pub fn downloaded(got: u64, total: Option<u64>, lang: Lang) -> String {
    let mb = |b: u64| format!("{:.1}", b as f64 / 1_048_576.0);
    match (total, is_ar(lang)) {
        (Some(t), false) => format!("{} of {} MB", mb(got), mb(t)),
        (Some(t), true) => format!("{} من {} ميجابايت", mb(got), mb(t)),
        (None, false) => format!("{} MB", mb(got)),
        (None, true) => format!("{} ميجابايت", mb(got)),
    }
}

/// A line speed with its unit: "30 Mbps", "256 Kbps".
pub fn speed(kbps: u32, lang: Lang) -> String {
    let (n, mbps) = format::speed(kbps);
    match (lang, mbps) {
        (Lang::En, true) => format!("{n} Mbps"),
        (Lang::En, false) => format!("{n} Kbps"),
        (Lang::Ar, true) => format!("{n} ميجابت/ث"),
        (Lang::Ar, false) => format!("{n} كيلوبت/ث"),
    }
}

/// "Up to 30 Mbps", beside a package's name.
pub fn up_to(kbps: u32, lang: Lang) -> String {
    let speed = speed(kbps, lang);
    if is_ar(lang) { format!("حتى {speed}") } else { format!("Up to {speed}") }
}

/// "30 Mbps down, 3 Mbps up".
pub fn speeds(s: &klue_core::model::Speed, lang: Lang) -> String {
    let down = speed(s.down_kbps, lang);
    match (s.up_kbps, is_ar(lang)) {
        (Some(up), false) => format!("{down} down, {} up", speed(up, lang)),
        (Some(up), true) => format!("{down} تحميل، {} رفع", speed(up, lang)),
        (None, _) => down,
    }
}

/// "15 min" / "15 د".
pub fn every_label(every: klue_core::store::RefreshEvery, lang: Lang) -> &'static str {
    use klue_core::store::RefreshEvery as R;
    match (every, is_ar(lang)) {
        (R::Minutes15, false) => "15 min",
        (R::Hour1, false) => "1 hr",
        (R::Hours3, false) => "3 hr",
        (R::Hours6, false) => "6 hr",
        (R::Minutes15, true) => "15 د",
        (R::Hour1, true) => "ساعة",
        (R::Hours3, true) => "3 س",
        (R::Hours6, true) => "6 س",
    }
}

pub fn is_ar(lang: Lang) -> bool {
    lang == Lang::Ar
}

pub fn carrier_name(c: Carrier, lang: Lang) -> &'static str {
    let ar = is_ar(lang);
    match c {
        Carrier::We => if ar { "وي" } else { "WE" },
        Carrier::Vodafone => if ar { "فودافون" } else { "Vodafone" },
        Carrier::Orange => if ar { "أورنج" } else { "Orange" },
        Carrier::Etisalat => if ar { "إي آند" } else { "e&" },
    }
}

/// The carrier's own app, where the same password works.
pub fn carrier_app(c: Carrier, lang: Lang) -> &'static str {
    let ar = is_ar(lang);
    match c {
        Carrier::We => "My WE",
        Carrier::Vodafone => if ar { "أنا فودافون" } else { "Ana Vodafone" },
        Carrier::Orange => "My Orange",
        Carrier::Etisalat => if ar { "تطبيق e&" } else { "e& app" },
    }
}

/// A kind of line in the Add wizard: its name and what it is.
pub fn product(p: Product, lang: Lang) -> (&'static str, &'static str) {
    use Product::*;
    let (en, ar) = match p {
        WeHomeInternet => (("Home internet", "Your home internet quota (ADSL or VDSL)"), ("الإنترنت المنزلي", "باقة الإنترنت المنزلي (ADSL أو VDSL)")),
        WeOneHome => (("OneHome", "Home internet and Ardy on one account"), ("وان هوم", "الإنترنت المنزلي والأرضي على حساب واحد")),
        WeArdy => (("Ardy landline", "Your home phone line"), ("الأرضي", "خط التليفون المنزلي")),
        WeMobile => (("WE Mobile", "Mobile minutes and internet"), ("وي موبايل", "دقائق وإنترنت المحمول")),
        WeAir => (("WE Air", "4G home internet"), ("وي إير", "إنترنت منزلي 4G")),
        WeGold => (("WE Gold", "Mobile minutes, home internet or WE Air"), ("وي جولد", "دقائق المحمول أو الإنترنت المنزلي أو وي إير")),
        VfMobile => (("Mobile", "Your Vodafone mobile line"), ("المحمول", "خط فودافون المحمول")),
        VfHomeInternet => (("Home internet", "Vodafone home internet (DSL)"), ("الإنترنت المنزلي", "إنترنت فودافون المنزلي (DSL)")),
        VfUsb => (("USB and Mobile Wi‑Fi", "Mobile internet on a modem or router"), ("USB و Mobile Wi‑Fi", "إنترنت المحمول على مودم أو راوتر")),
        OrMobile => (("Mobile", "Your Orange mobile line"), ("المحمول", "خط أورنج المحمول")),
        OrHomeInternet => (("Home internet", "Orange home internet (DSL), through My Orange"), ("الإنترنت المنزلي", "إنترنت أورنج المنزلي (DSL) من خلال My Orange")),
        EaMobile => (("Mobile", "Your e& mobile line"), ("المحمول", "خط إي آند المحمول")),
        EaHomeInternet => (("Home internet", "e& home internet (DSL)"), ("الإنترنت المنزلي", "إنترنت إي آند المنزلي (DSL)")),
    };
    if is_ar(lang) { ar } else { en }
}

/// The sign-in step's title.
pub fn sign_in_title(app: &str, lang: Lang) -> String {
    if is_ar(lang) { format!("سجّل الدخول بحساب {app}") } else { format!("Sign in with {app}") }
}

/// The number a carrier's app signs in with, when it isn't the line's own.
pub fn app_number(app: &str, lang: Lang) -> String {
    if is_ar(lang) { format!("رقم {app}") } else { format!("{app} number") }
}

pub fn same_password(app: &str, lang: Lang) -> String {
    if is_ar(lang) { format!("نفس كلمة المرور المستخدمة في {app}.") } else { format!("The same password you use in {app}.") }
}

/// Under the number of an Orange home internet line.
pub fn linked_landline_hint(app: &str, lang: Lang) -> String {
    if is_ar(lang) {
        format!("رقم المحمول الذي يستخدمه حسابك في {app}. يقرأ Klue الخط الأرضي المربوط به.")
    } else {
        format!("The mobile number your {app} account uses. Klue reads the landline linked to it.")
    }
}

pub fn signing_in(carrier: Carrier, lang: Lang) -> String {
    let name = carrier_name(carrier, lang);
    if is_ar(lang) { format!("جارٍ تسجيل الدخول إلى {name}…") } else { format!("Signing in to {name}…") }
}

/// The finish step's offer of the account's other line.
pub fn also_has(line: LineType, lang: Lang) -> &'static str {
    match (line, is_ar(lang)) {
        (LineType::Landline, false) => "This account also has home internet",
        (LineType::Mobile, false) => "This account also has a mobile line",
        (LineType::Voice, false) => "This account also has a landline",
        (LineType::Landline, true) => "هذا الحساب عليه إنترنت منزلي أيضاً",
        (LineType::Mobile, true) => "هذا الحساب عليه خط محمول أيضاً",
        (LineType::Voice, true) => "هذا الحساب عليه خط أرضي أيضاً",
    }
}

/// Why an export file couldn't be used.
pub fn transfer_error(e: &klue_core::transfer::TransferError, lang: Lang) -> &'static str {
    use klue_core::transfer::TransferError as E;
    match (e, is_ar(lang)) {
        (E::NotAnExport, false) => "This isn’t a Klue export file.",
        (E::TooNew, false) => "This file comes from a newer Klue. Update Klue, then import it.",
        (E::WrongPassphrase, false) => "That passphrase doesn’t open the passwords.",
        (E::Seal, false) => "The passwords couldn’t be sealed.",
        (E::NotAnExport, true) => "هذا ليس ملف تصدير من Klue.",
        (E::TooNew, true) => "هذا الملف من إصدار أحدث من Klue. حدّث Klue ثم استورده.",
        (E::WrongPassphrase, true) => "عبارة المرور هذه لا تفتح كلمات المرور.",
        (E::Seal, true) => "تعذّرت حماية كلمات المرور.",
    }
}

/// Export: how many of the numbers go in the file.
pub fn picked_of(picked: usize, of: usize, lang: Lang) -> String {
    if is_ar(lang) { format!("تم تحديد {picked} من {of}") } else { format!("{picked} of {of} selected") }
}

pub fn saved_to(path: &str, lang: Lang) -> String {
    if is_ar(lang) { format!("حُفظ في {path}") } else { format!("Saved to {path}") }
}

/// Under an export file's name.
pub fn exported_on(at: DateTime<Utc>, version: &str, lang: Lang) -> String {
    let day = date_short(at, lang);
    if is_ar(lang) { format!("صُدّر في {day} من Klue {version}") } else { format!("Exported {day} from Klue {version}") }
}

/// "3 lines" under a group's name.
pub fn lines_count(n: usize, lang: Lang) -> String {
    match (lang, n) {
        (Lang::En, 0) => "No lines".into(),
        (Lang::En, 1) => "1 line".into(),
        (Lang::En, n) => format!("{n} lines"),
        (Lang::Ar, 0) => "لا خطوط".into(),
        (Lang::Ar, 1) => "خط واحد".into(),
        (Lang::Ar, 2) => "خطان".into(),
        (Lang::Ar, 3..=10) => format!("{n} خطوط"),
        (Lang::Ar, n) => format!("{n} خط"),
    }
}

/// "Renews in 13 days" on a home card.
pub fn renews_in(at: DateTime<Utc>, now: DateTime<Utc>, lang: Lang) -> String {
    let days = (at - now).num_days().max(0);
    match (lang, days) {
        (Lang::En, 0) => "Renews today".into(),
        (Lang::En, _) => format!("Renews in {}", count(days, Span::Day, lang)),
        (Lang::Ar, 0) => "يتجدد اليوم".into(),
        (Lang::Ar, _) => format!("يتجدد خلال {}", count(days, Span::Day, lang)),
    }
}

/// "Ends in 5 days" on a home card for a bundle with its own end.
pub fn ends_in(at: DateTime<Utc>, now: DateTime<Utc>, lang: Lang) -> String {
    let days = (at - now).num_days().max(0);
    match (lang, days) {
        (Lang::En, 0) => "Ends today".into(),
        (Lang::En, _) => format!("Ends in {}", count(days, Span::Day, lang)),
        (Lang::Ar, 0) => "تنتهي اليوم".into(),
        (Lang::Ar, _) => format!("تنتهي خلال {}", count(days, Span::Day, lang)),
    }
}

/// The finish step's button for three numbers or more.
pub fn add_numbers(n: usize, lang: Lang) -> String {
    if is_ar(lang) { format!("إضافة {n} أرقام") } else { format!("Add {n} numbers") }
}

/// "388 units left".
pub fn left(amount: &str, lang: Lang) -> String {
    if is_ar(lang) { format!("متبقي {amount}") } else { format!("{amount} left") }
}

pub fn line_name(line: LineType, lang: Lang) -> &'static str {
    let s = strings(lang);
    match line {
        LineType::Mobile => s.mobile,
        LineType::Landline => s.landline,
        LineType::Voice => s.voice,
    }
}

pub fn kind_name(kind: BundleKind, lang: Lang) -> &'static str {
    let ar = is_ar(lang);
    match kind {
        BundleKind::Data => if ar { "إنترنت" } else { "Data" },
        BundleKind::Addon => if ar { "إضافة" } else { "Add-on" },
        BundleKind::Minutes => if ar { "دقائق" } else { "Minutes" },
        BundleKind::Social => if ar { "سوشيال" } else { "Social data" },
        BundleKind::Flex => if ar { "فليكس" } else { "Flex" },
        BundleKind::Sms => if ar { "رسائل" } else { "SMS" },
        BundleKind::Other => if ar { "باقة" } else { "Bundle" },
    }
}

pub fn unit_name(unit: Unit, lang: Lang) -> &'static str {
    let ar = is_ar(lang);
    match unit {
        Unit::Gb => if ar { "جيجا" } else { "GB" },
        Unit::Mb => if ar { "ميجا" } else { "MB" },
        Unit::Minutes => if ar { "دقيقة" } else { "min" },
        Unit::Sms => if ar { "رسالة" } else { "SMS" },
        Unit::Flex => if ar { "فليكس" } else { "Flex" },
        Unit::Units => if ar { "وحدة" } else { "units" },
        Unit::Egp => if ar { "جنيه" } else { "EGP" },
    }
}

/// "GB left" under the big number.
pub fn left_label(unit: Unit, lang: Lang) -> String {
    match lang {
        Lang::En => format!("{} left", unit_name(unit, lang)),
        Lang::Ar => match unit {
            Unit::Gb => "جيجا متبقية".into(),
            Unit::Minutes => "دقيقة متبقية".into(),
            Unit::Sms => "رسالة متبقية".into(),
            Unit::Units => "وحدة متبقية".into(),
            _ => format!("{} متبقي", unit_name(unit, lang)),
        },
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Minute,
    Hour,
    Day,
}

/// "2 min" / "دقيقتين", with Arabic dual and plural forms.
pub fn count(n: i64, span: Span, lang: Lang) -> String {
    match lang {
        Lang::En => match span {
            Span::Minute => format!("{n} min"),
            Span::Hour => format!("{n} {}", if n == 1 { "hour" } else { "hours" }),
            Span::Day => format!("{n} {}", if n == 1 { "day" } else { "days" }),
        },
        Lang::Ar => {
            let (one, two, few, many) = match span {
                Span::Minute => ("دقيقة", "دقيقتين", "دقائق", "دقيقة"),
                Span::Hour => ("ساعة", "ساعتين", "ساعات", "ساعة"),
                Span::Day => ("يوم", "يومين", "أيام", "يوم"),
            };
            match n {
                1 => one.into(),
                2 => two.into(),
                3..=10 => format!("{n} {few}"),
                _ => format!("{n} {many}"),
            }
        }
    }
}

pub fn synced(last: Option<DateTime<Utc>>, now: DateTime<Utc>, lang: Lang) -> String {
    let s = strings(lang);
    let Some(last) = last else { return s.not_synced.into() };
    let mins = (now - last).num_minutes().max(0);
    let ago = if mins < 1 {
        return s.synced_now.into();
    } else if mins < 60 {
        count(mins, Span::Minute, lang)
    } else if mins < 48 * 60 {
        count(mins / 60, Span::Hour, lang)
    } else {
        count(mins / (24 * 60), Span::Day, lang)
    };
    match lang {
        Lang::En => format!("Synced {ago} ago"),
        Lang::Ar => format!("آخر تحديث منذ {ago}"),
    }
}

const MONTHS_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const MONTHS_AR: [&str; 12] = ["يناير", "فبراير", "مارس", "أبريل", "مايو", "يونيو", "يوليو", "أغسطس", "سبتمبر", "أكتوبر", "نوفمبر", "ديسمبر"];
const DAYS_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const DAYS_AR: [&str; 7] = ["الاثنين", "الثلاثاء", "الأربعاء", "الخميس", "الجمعة", "السبت", "الأحد"];

/// "Borrowed" / "Frozen" for a bundle that isn't simply usable.
pub fn state_name(state: BundleState, lang: Lang) -> Option<&'static str> {
    let s = strings(lang);
    match state {
        BundleState::Active => None,
        BundleState::Borrowed => Some(s.borrowed),
        BundleState::Frozen => Some(s.frozen),
        BundleState::Inactive => Some(s.inactive),
    }
}

/// "104.50 EGP owed" / "عليك 104.50 جنيه", under the balance.
pub fn owed(amount: f64, lang: Lang) -> String {
    let m = format::money(amount);
    match lang {
        Lang::En => format!("{m} EGP owed"),
        Lang::Ar => format!("عليك {m} جنيه"),
    }
}

/// "Settle by 10 Dec" / "سدد قبل 10 ديسمبر", for the tray.
pub fn settle_by_short(dt: DateTime<Utc>, lang: Lang) -> String {
    let d = dt.with_timezone(&Local);
    let m = d.month0() as usize;
    match lang {
        Lang::En => format!("Settle by {} {}", d.day(), &MONTHS_EN[m][..3]),
        Lang::Ar => format!("سدد قبل {} {}", d.day(), MONTHS_AR[m]),
    }
}

/// Notice title and text for a borrowed or frozen bundle. `what` is the package, or the
/// bundle's name when it isn't the main one.
pub fn state_notice(
    state: BundleState,
    carrier: Carrier,
    what: &str,
    owed: Option<f64>,
    settle_by: Option<DateTime<Utc>>,
    lang: Lang,
) -> Option<(String, String)> {
    let ar = is_ar(lang);
    let name = carrier_name(carrier, lang);
    let pay = match (owed.map(format::money), ar) {
        (Some(a), false) => format!("Settle {a} EGP"),
        (Some(a), true) => format!("سدد {a} جنيه"),
        (None, false) => "Settle it".to_string(),
        (None, true) => "سدد قيمتها".to_string(),
    };
    match (state, ar) {
        (BundleState::Active, _) => None,
        (BundleState::Borrowed, false) => {
            let by = settle_by.map(|d| format!(" by {}", date_long(d, lang))).unwrap_or_default();
            Some((format!("{what} is borrowed"), format!("{name} renewed it on credit. {pay}{by}, or it will be frozen.")))
        }
        (BundleState::Borrowed, true) => {
            let by = settle_by.map(|d| format!(" قبل {}", date_long(d, lang))).unwrap_or_default();
            Some((format!("باقة {what} مستلفة"), format!("جددتها {name} بالسلف. {pay}{by} لتجنب إيقاف الباقة.")))
        }
        (BundleState::Frozen, false) => {
            Some((format!("{what} is frozen"), format!("It was renewed on credit and not paid for in time. {pay} to use it again.")))
        }
        (BundleState::Frozen, true) => {
            Some((format!("باقة {what} موقوفة"), format!("جُددت بالسلف ولم تُسدَّد قيمتها في موعدها. {pay} لتستخدمها مرة أخرى.")))
        }
        (BundleState::Inactive, false) => Some((format!("{what} isn’t active"), "It renews when you recharge the line.".to_string())),
        (BundleState::Inactive, true) => Some((format!("باقة {what} غير مفعّلة"), "تتجدد الباقة عند شحن رصيد الخط.".to_string())),
    }
}

/// "Tue, 8 December 2026" / "الثلاثاء، 8 ديسمبر 2026".
pub fn date_long(dt: DateTime<Utc>, lang: Lang) -> String {
    let d = dt.with_timezone(&Local);
    let (m, wd) = (d.month0() as usize, d.weekday().num_days_from_monday() as usize);
    match lang {
        Lang::En => format!("{}, {} {} {}", DAYS_EN[wd], d.day(), MONTHS_EN[m], d.year()),
        Lang::Ar => format!("{}، {} {} {}", DAYS_AR[wd], d.day(), MONTHS_AR[m], d.year()),
    }
}

/// "08 Dec 2026" / "08 ديسمبر 2026".
pub fn date_short(dt: DateTime<Utc>, lang: Lang) -> String {
    let d = dt.with_timezone(&Local);
    let m = d.month0() as usize;
    match lang {
        Lang::En => format!("{:02} {} {}", d.day(), &MONTHS_EN[m][..3], d.year()),
        Lang::Ar => format!("{:02} {} {}", d.day(), MONTHS_AR[m], d.year()),
    }
}

/// "in 16 days, 14 hrs" / "بعد 16 يوم و14 ساعة".
pub fn countdown(until: DateTime<Utc>, now: DateTime<Utc>, lang: Lang) -> String {
    let secs = (until - now).num_seconds();
    if secs <= 0 {
        return match lang {
            Lang::En => "Renewing now".into(),
            Lang::Ar => "يتجدد الآن".into(),
        };
    }
    let (d, h) = (secs / 86_400, (secs % 86_400) / 3_600);
    match lang {
        Lang::En => {
            let days = if d == 1 { "1 day".to_string() } else { format!("{d} days") };
            let hours = if h == 1 { "1 hr".to_string() } else { format!("{h} hrs") };
            match (d, h) {
                (0, _) => format!("in {hours}"),
                (_, 0) => format!("in {days}"),
                _ => format!("in {days}, {hours}"),
            }
        }
        Lang::Ar => {
            let (days, hours) = (count(d, Span::Day, lang), count(h, Span::Hour, lang));
            match (d, h) {
                (0, _) => format!("بعد {hours}"),
                (_, 0) => format!("بعد {days}"),
                _ => format!("بعد {days} و{hours}"),
            }
        }
    }
}

/// Explanation of a failed refresh or sign-in.
pub fn error_text(e: &FetchError, carrier: Carrier, c: Clock, lang: Lang) -> String {
    let name = carrier_name(carrier, lang);
    let app = carrier_app(carrier, lang);
    let ar = is_ar(lang);
    match e {
        FetchError::BadCredentials => {
            if ar {
                format!("{name} رفضت هذا الرقم وكلمة المرور. تأكد منها في {app} ثم حاول مرة أخرى.")
            } else {
                format!("{name} didn’t accept this number and password. Check it in {app}, then try again.")
            }
        }
        FetchError::CaptchaUnsolved => {
            if ar {
                format!("طلبت {name} رمز تحقق (كابتشا) ولم يتمكن Klue من قراءته. اضغط تحديث للمحاولة مرة أخرى، وإن تعذّر ذلك فسيطلب منك كتابته.")
            } else {
                format!("{name} asked for a captcha and Klue couldn’t read it. Press Refresh to try again; if it still can’t, it will ask you to type it.")
            }
        }
        FetchError::CaptchaRejected => {
            if ar {
                format!("لم تقبل {name} رمز التحقق الذي قرأه Klue. سيحاول مرة أخرى برمز جديد عند التحديث التالي.")
            } else {
                format!("{name} didn’t accept the captcha code Klue read. It will try again with a new captcha at the next refresh.")
            }
        }
        FetchError::CaptchaBlocked(until) => {
            let t = clock(*until, c, lang);
            if ar {
                format!("أوقفت {name} تسجيل الدخول مؤقتاً بعد محاولات كثيرة. سيحاول Klue مرة أخرى بعد {t}.")
            } else {
                format!("{name} paused sign-ins after too many attempts. Klue will try again after {t}.")
            }
        }
        FetchError::Carrier(msg) => {
            if ar { format!("رسالة من {name}: {msg}") } else { format!("{name} said: {msg}") }
        }
        FetchError::Network(_) => {
            if ar {
                format!("تعذّر الاتصال بـ{name}. تأكد من اتصال الإنترنت.")
            } else {
                format!("Couldn’t reach {name}. Check your internet connection.")
            }
        }
        FetchError::Unexpected(detail) => {
            if ar {
                format!("ردّت {name} بشكل غير متوقع ({detail}).")
            } else {
                format!("{name} sent a response Klue didn’t understand ({detail}).")
            }
        }
        FetchError::InvalidNumber(n) => {
            if ar { format!("الرقم \"{n}\" غير صحيح.") } else { format!("\u{201c}{n}\u{201d} doesn’t look like a valid number.") }
        }
        FetchError::NoLine(LineType::Mobile) => {
            if ar {
                format!("لا يوجد خط محمول من {name} على حساب {app} هذا. إن كان للإنترنت المنزلي فاختر DSL.")
            } else {
                format!("This {app} account has no {name} mobile line. If it’s for home internet, choose DSL.")
            }
        }
        FetchError::NoLine(LineType::Voice) => {
            if ar {
                format!("لا يوجد خط أرضي من {name} على حساب {app} هذا.")
            } else {
                format!("This {app} account has no {name} landline.")
            }
        }
        FetchError::NoLine(LineType::Landline) => {
            if ar {
                format!("لا يوجد خط أرضي مربوط بحساب {app} هذا، أو صار مربوطاً بخط آخر. اربط الخط في {app} ثم حاول مرة أخرى.")
            } else {
                format!("No landline is linked to this {app} account, or it’s linked to another one now. Link the line in {app}, then try again.")
            }
        }
        FetchError::Unsupported => strings(lang).coming_soon_body.into(),
        FetchError::AlreadyAdded { archived: false } => strings(lang).already_added.into(),
        FetchError::AlreadyAdded { archived: true } => strings(lang).already_archived.into(),
        FetchError::NoPassword => strings(lang).no_password.into(),
    }
}

/// Error title in the Add dialog.
pub fn error_title(e: &FetchError, lang: Lang) -> &'static str {
    let ar = is_ar(lang);
    match e {
        FetchError::BadCredentials => strings(lang).err_title,
        FetchError::CaptchaUnsolved => if ar { "مطلوب رمز تحقق" } else { "Captcha needed" },
        FetchError::CaptchaBlocked(_) => if ar { "تسجيل الدخول متوقف مؤقتاً" } else { "Sign-ins paused" },
        FetchError::CaptchaRejected => if ar { "لم يُقبل رمز التحقق" } else { "Captcha not accepted" },
        FetchError::InvalidNumber(_) => if ar { "رقم غير صحيح" } else { "Check the number" },
        FetchError::NoLine(_) => if ar { "الخط غير موجود" } else { "Line not found" },
        FetchError::Network(_) => if ar { "لا يوجد اتصال" } else { "No connection" },
        FetchError::AlreadyAdded { .. } => if ar { "مضاف بالفعل" } else { "Already in Klue" },
        _ => if ar { "تعذّر التحقق" } else { "Couldn’t verify" },
    }
}

// ---------------------------------------------------------------------------------------
// The usage graph

/// "Mon 23 Nov" / "الاثنين 23 نوفمبر", in local time.
pub fn day_label(dt: DateTime<Utc>, lang: Lang) -> String {
    let d = dt.with_timezone(&Local);
    let (m, wd) = (d.month0() as usize, d.weekday().num_days_from_monday() as usize);
    match lang {
        Lang::En => format!("{} {} {}", DAYS_EN[wd], d.day(), &MONTHS_EN[m][..3]),
        Lang::Ar => format!("{} {} {}", DAYS_AR[wd], d.day(), MONTHS_AR[m]),
    }
}

/// "14:07", or "2:07 PM" / "2:07 م", in local time.
pub fn clock(dt: DateTime<Utc>, clock: Clock, lang: Lang) -> String {
    let t = dt.with_timezone(&Local);
    match clock {
        Clock::H24 => t.format("%H:%M").to_string(),
        Clock::H12 => format!("{}:{:02} {}", hour12(t.hour()), t.minute(), am_pm(t.hour(), lang)),
    }
}

/// An hour on the graph's time axis: "14:00", or "2 PM" / "2 م".
pub fn hour_label(dt: DateTime<Utc>, clock: Clock, lang: Lang) -> String {
    let t = dt.with_timezone(&Local);
    match clock {
        Clock::H24 => t.format("%H:%M").to_string(),
        Clock::H12 => format!("{} {}", hour12(t.hour()), am_pm(t.hour(), lang)),
    }
}

fn hour12(hour: u32) -> u32 {
    match hour % 12 {
        0 => 12,
        h => h,
    }
}

fn am_pm(hour: u32, lang: Lang) -> &'static str {
    match (hour < 12, lang) {
        (true, Lang::En) => "AM",
        (false, Lang::En) => "PM",
        (true, Lang::Ar) => "ص",
        (false, Lang::Ar) => "م",
    }
}

/// "Mon 23 Nov, 14:07" / "الاثنين 23 نوفمبر، 14:07".
pub fn moment(dt: DateTime<Utc>, c: Clock, lang: Lang) -> String {
    match lang {
        Lang::En => format!("{}, {}", day_label(dt, lang), clock(dt, c, lang)),
        Lang::Ar => format!("{}، {}", day_label(dt, lang), clock(dt, c, lang)),
    }
}

/// "Mon 23 Nov, 14:00–15:00".
pub fn hour_span(start: DateTime<Utc>, end: DateTime<Utc>, c: Clock, lang: Lang) -> String {
    match lang {
        Lang::En => format!("{}, {}–{}", day_label(start, lang), clock(start, c, lang), clock(end, c, lang)),
        Lang::Ar => format!("{}، {}–{}", day_label(start, lang), clock(start, c, lang), clock(end, c, lang)),
    }
}

/// Under a day on the time axis: "23 Nov" (with the month) or "23".
pub fn axis_day(dt: DateTime<Utc>, with_month: bool, lang: Lang) -> String {
    let d = dt.with_timezone(&Local);
    let m = d.month0() as usize;
    match (with_month, lang) {
        (false, _) => d.day().to_string(),
        (true, Lang::En) => format!("{} {}", d.day(), &MONTHS_EN[m][..3]),
        (true, Lang::Ar) => format!("{} {}", d.day(), MONTHS_AR[m]),
    }
}

/// With its unit; GB under 1,000 MB is shown in MB: "3.2 GB", "350 MB".
pub fn quantity(v: f64, unit: Unit, lang: Lang) -> String {
    let (v, unit) = if unit == Unit::Gb && v != 0.0 && (v * 1024.0).abs() < 999.5 { (v * 1024.0, Unit::Mb) } else { (v, unit) };
    format!("{} {}", format::amount_short(v, unit), unit_name(unit, lang))
}

/// "87.4 GB left" / "متبقي 87.4 جيجا".
pub fn left_of(v: f64, unit: Unit, lang: Lang) -> String {
    let q = quantity(v, unit, lang);
    if is_ar(lang) { format!("متبقي {q}") } else { format!("{q} left") }
}

/// "3.2 GB used" / "تم استخدام 3.2 جيجا".
pub fn used_of(v: f64, unit: Unit, lang: Lang) -> String {
    let q = quantity(v, unit, lang);
    if is_ar(lang) { format!("تم استخدام {q}") } else { format!("{q} used") }
}

/// "1.2 GB since 11:52".
pub fn used_since(v: f64, unit: Unit, since: DateTime<Utc>, c: Clock, lang: Lang) -> String {
    let q = quantity(v, unit, lang);
    let t = clock(since, c, lang);
    if is_ar(lang) { format!("{q} منذ {t}") } else { format!("{q} since {t}") }
}

/// "Spread over a 9-hour wait between refreshes".
pub fn spread_over(gap: chrono::Duration, lang: Lang) -> String {
    let hours = (gap.num_minutes() as f64 / 60.0).round().max(1.0) as i64;
    let (n, span) = if hours >= 48 { (hours / 24, Span::Day) } else { (hours, Span::Hour) };
    match lang {
        Lang::En => format!("Spread over a {n}-{} wait between refreshes", if span == Span::Day { "day" } else { "hour" }),
        Lang::Ar => format!("موزّع على انتظار {} بين تحديثين", count(n, span, lang)),
    }
}

/// "From 09:12" / "Until 14:07".
pub fn from_clock(t: DateTime<Utc>, c: Clock, lang: Lang) -> String {
    let t = clock(t, c, lang);
    if is_ar(lang) { format!("من {t}") } else { format!("From {t}") }
}

pub fn until_clock(t: DateTime<Utc>, c: Clock, lang: Lang) -> String {
    let t = clock(t, c, lang);
    if is_ar(lang) { format!("حتى {t}") } else { format!("Until {t}") }
}

/// What an average is over: "Over 16 days" / "على مدى 16 يوم".
pub fn over_days(days: f64, lang: Lang) -> String {
    let (n, span) = if days >= 1.0 { (days.round() as i64, Span::Day) } else { ((days * 24.0).round().max(1.0) as i64, Span::Hour) };
    let c = count(n, span, lang);
    if is_ar(lang) { format!("على مدى {c}") } else { format!("Over {c}") }
}

/// "Runs out Thu 3 Dec".
pub fn runs_out(at: DateTime<Utc>, lang: Lang) -> String {
    let day = day_label(at, lang);
    if is_ar(lang) { format!("ينفد {day}") } else { format!("Runs out {day}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn arabic_counts() {
        assert_eq!(count(2, Span::Minute, Lang::Ar), "دقيقتين");
        assert_eq!(count(5, Span::Minute, Lang::Ar), "5 دقائق");
        assert_eq!(count(15, Span::Minute, Lang::Ar), "15 دقيقة");
        assert_eq!(count(2, Span::Minute, Lang::En), "2 min");
    }

    #[test]
    fn date_formats() {
        let dt = Local.with_ymd_and_hms(2026, 12, 8, 12, 0, 0).unwrap().with_timezone(&Utc);
        assert_eq!(date_long(dt, Lang::En), "Tue, 8 December 2026");
        assert_eq!(date_long(dt, Lang::Ar), "الثلاثاء، 8 ديسمبر 2026");
        assert_eq!(date_short(dt, Lang::En), "08 Dec 2026");
        assert_eq!(date_short(dt, Lang::Ar), "08 ديسمبر 2026");
    }

    #[test]
    fn clock_formats() {
        let at = |h, m| Local.with_ymd_and_hms(2026, 11, 25, h, m, 0).unwrap().with_timezone(&Utc);
        assert_eq!(clock(at(14, 7), Clock::H24, Lang::En), "14:07");
        assert_eq!(clock(at(14, 7), Clock::H12, Lang::En), "2:07 PM");
        assert_eq!(clock(at(0, 57), Clock::H12, Lang::En), "12:57 AM");
        assert_eq!(clock(at(12, 0), Clock::H12, Lang::En), "12:00 PM");
        assert_eq!(clock(at(9, 5), Clock::H12, Lang::Ar), "9:05 ص");
        assert_eq!(clock(at(21, 5), Clock::H12, Lang::Ar), "9:05 م");
        assert_eq!(hour_label(at(14, 0), Clock::H12, Lang::En), "2 PM");
        assert_eq!(hour_label(at(0, 0), Clock::H12, Lang::En), "12 AM");
        assert_eq!(hour_label(at(14, 0), Clock::H24, Lang::Ar), "14:00");
        assert_eq!(moment(at(0, 57), Clock::H12, Lang::En), "Wed 25 Nov, 12:57 AM");
        assert_eq!(used_since(1.6, Unit::Gb, at(23, 56), Clock::H12, Lang::En), "1.6 GB since 11:56 PM");
    }

    #[test]
    fn countdowns() {
        let now = Utc.with_ymd_and_hms(2026, 11, 21, 10, 0, 0).unwrap();
        let until = Utc.with_ymd_and_hms(2026, 12, 8, 0, 0, 0).unwrap();
        assert_eq!(countdown(until, now, Lang::En), "in 16 days, 14 hrs");
        assert_eq!(countdown(until, now, Lang::Ar), "بعد 16 يوم و14 ساعة");
        let soon = now + chrono::Duration::hours(31);
        assert_eq!(countdown(soon, now, Lang::En), "in 1 day, 7 hrs");
        assert_eq!(countdown(soon, now, Lang::Ar), "بعد يوم و7 ساعات");
        assert_eq!(countdown(now + chrono::Duration::hours(5), now, Lang::En), "in 5 hrs");
        assert_eq!(countdown(now + chrono::Duration::days(3), now, Lang::Ar), "بعد 3 أيام");
    }

    #[test]
    fn usage_graph_wording() {
        assert_eq!(quantity(0.25, Unit::Gb, Lang::En), "256 MB");
        assert_eq!(quantity(0.984, Unit::Gb, Lang::En), "1 GB");
        assert_eq!(quantity(3.26, Unit::Gb, Lang::En), "3.3 GB");
        assert_eq!(left_of(87.4, Unit::Gb, Lang::Ar), "متبقي 87.4 جيجا");
        assert_eq!(spread_over(chrono::Duration::minutes(32 * 60 + 10), Lang::En), "Spread over a 32-hour wait between refreshes");
        assert_eq!(spread_over(chrono::Duration::hours(50), Lang::En), "Spread over a 2-day wait between refreshes");
        assert_eq!(over_days(16.4, Lang::En), "Over 16 days");
        assert_eq!(over_days(0.25, Lang::Ar), "على مدى 6 ساعات");
    }

    #[test]
    fn borrowed_and_frozen_notices() {
        let by = Local.with_ymd_and_hms(2026, 12, 8, 12, 0, 0).unwrap().with_timezone(&Utc);
        let notice = |state, owed, settle_by, lang| state_notice(state, Carrier::Vodafone, "Flex 115", owed, settle_by, lang).unwrap();
        assert_eq!(
            notice(BundleState::Borrowed, Some(104.5), Some(by), Lang::En),
            ("Flex 115 is borrowed".into(), "Vodafone renewed it on credit. Settle 104.50 EGP by Tue, 8 December 2026, or it will be frozen.".into())
        );
        assert_eq!(notice(BundleState::Borrowed, None, None, Lang::En).1, "Vodafone renewed it on credit. Settle it, or it will be frozen.");
        assert_eq!(
            notice(BundleState::Borrowed, Some(104.5), Some(by), Lang::Ar),
            ("باقة Flex 115 مستلفة".into(), "جددتها فودافون بالسلف. سدد 104.50 جنيه قبل الثلاثاء، 8 ديسمبر 2026 لتجنب إيقاف الباقة.".into())
        );
        assert_eq!(
            notice(BundleState::Frozen, Some(104.5), None, Lang::En),
            ("Flex 115 is frozen".into(), "It was renewed on credit and not paid for in time. Settle 104.50 EGP to use it again.".into())
        );
        assert_eq!(notice(BundleState::Frozen, None, None, Lang::Ar).1, "جُددت بالسلف ولم تُسدَّد قيمتها في موعدها. سدد قيمتها لتستخدمها مرة أخرى.");
        assert!(state_notice(BundleState::Active, Carrier::Vodafone, "x", None, None, Lang::En).is_none());

        assert_eq!((owed(1685.5, Lang::En), owed(79.8, Lang::Ar)), ("1,685.50 EGP owed".into(), "عليك 79.80 جنيه".into()));
        assert_eq!((settle_by_short(by, Lang::En), settle_by_short(by, Lang::Ar)), ("Settle by 8 Dec".into(), "سدد قبل 8 ديسمبر".into()));
        assert_eq!(state_name(BundleState::Frozen, Lang::Ar), Some("موقوفة"));
        assert_eq!(state_name(BundleState::Active, Lang::En), None);
    }
}
