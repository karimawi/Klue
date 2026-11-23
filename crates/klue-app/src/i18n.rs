//! English and Arabic strings.

use chrono::{DateTime, Datelike, Local, Utc};
use klue_core::{BundleKind, BundleState, Carrier, FetchError, LineType, Unit, format, store::Lang};

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
    pub add_title: &'static str,
    pub carrier: &'static str,
    pub line_type: &'static str,
    pub number: &'static str,
    pub password: &'static str,
    pub owner_name: &'static str,
    pub from_account: &'static str,
    pub auto_fill: &'static str,
    pub fetching: &'static str,
    pub nick_optional: &'static str,
    pub nick_ph: &'static str,
    pub verify: &'static str,
    pub verifying: &'static str,
    pub cancel: &'static str,
    pub verified: &'static str,
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
    pub owed_note: &'static str,
    /// Add dialog tooltips for line types that aren't supported yet.
    pub we_mobile_soon: &'static str,
    pub vf_dsl_soon: &'static str,
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
    add_number: "Add number", mobile: "Mobile", landline: "DSL", registered_to: "Registered to", refresh: "Refresh",
    rename: "Rename", alerts: "Alerts", package: "Package", below_alert: "Below alert", remaining: "remaining", change: "Change",
    next_renewal: "Next renewal", cycle: "Billing cycle", subscribed: "Subscribed", renews: "Renews", bundles: "Bundles & add-ons",
    col_bundle: "Bundle", col_rem: "Remaining", col_total: "Total", col_left: "Left", col_alert: "Alert", off: "Off",
    empty_body: "Add a number and its account password. Klue signs in, reads the quota, and fills in the registered owner for you. Mobile lines and DSL both work.",
    steps: ["Enter the number", "Enter the account password", "Owner name and quota load automatically"],
    new_number: "New number", add_title: "Add a number", carrier: "Carrier", line_type: "Line type", number: "Number",
    password: "Account password", owner_name: "Owner name", from_account: "From the account",
    auto_fill: "Filled in after the password is verified", fetching: "Reading account…", nick_optional: "Nickname (optional)",
    nick_ph: "e.g. Home internet", verify: "Verify password", verifying: "Verifying…", cancel: "Cancel", verified: "Password verified",
    try_again: "Try again", err_title: "Password not accepted", rename_title: "Rename number", nickname: "Nickname",
    nick_hint: "Only changes how the number appears in Klue.", shown_as: "Shown as", use_owner: "Use owner name", save: "Save",
    quota_alerts: "Quota alerts", renewal_reminder: "Renewal reminder", remind_me: "Remind me",
    before_renewal: "before renewal", days: "Days", hours: "Hours", renew_day: "Also notify on renewal day", preview: "Preview",
    theme: "Theme", match_hint: "Match carrier recolors the app to each carrier’s colors as you switch tabs.", language: "Language",
    check_every: "Check quotas every", startup: "Launch at startup", tray: "Keep running in the tray",
    disclaimer: "Unofficial app. Not affiliated with any carrier.", done: "Done", open_app: "Open Klue", settings_title: "Settings",
    empty_kicker: "No numbers",
    coming_soon_body: "Klue can’t sign in to this carrier yet. WE home internet and Vodafone mobile lines work today; Orange and e& are next.",
    not_synced: "Not synced yet", syncing: "Syncing…", synced_now: "Synced just now", waiting_first_sync: "Waiting for the first sync…", owner_unknown: "Not provided by the carrier", remove: "Remove number",
    remove_hint: "Removes the number and its saved password from this computer.", remove_confirm: "Click again to remove",
    captcha_title: "Type the characters", captcha_body: "WE wants to check you’re human before signing in. Type the characters in the picture.",
    captcha_ph: "Characters", continue_: "Continue", already_added: "This number is already in Klue.",
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
    owed_note: "Your next recharge pays it first.",
    we_mobile_soon: "WE mobile (015) lines are coming soon.",
    vf_dsl_soon: "Vodafone DSL is coming soon.",
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
    add_number: "إضافة رقم", mobile: "محمول", landline: "DSL", registered_to: "مسجّل باسم", refresh: "تحديث", rename: "تغيير الاسم",
    alerts: "التنبيهات", package: "الباقة", below_alert: "أقل من حد التنبيه", remaining: "متبقي", change: "تعديل",
    next_renewal: "التجديد القادم", cycle: "دورة الاشتراك", subscribed: "تاريخ الاشتراك", renews: "يتجدد", bundles: "الباقات والإضافات",
    col_bundle: "الباقة", col_rem: "المتبقي", col_total: "الإجمالي", col_left: "النسبة", col_alert: "التنبيه", off: "غير مفعّل",
    empty_body: "أضف الرقم وكلمة مرور الحساب. يسجّل Klue الدخول ويقرأ الباقة ويملأ اسم صاحب الخط تلقائياً. يدعم خطوط المحمول وDSL.",
    steps: ["أدخل الرقم", "أدخل كلمة مرور الحساب", "يظهر اسم صاحب الخط والرصيد تلقائياً"],
    new_number: "رقم جديد", add_title: "إضافة رقم", carrier: "الشركة", line_type: "نوع الخط", number: "الرقم",
    password: "كلمة مرور الحساب", owner_name: "اسم صاحب الخط", from_account: "من بيانات الحساب",
    auto_fill: "يُملأ تلقائياً بعد التحقق من كلمة المرور", fetching: "جارٍ قراءة الحساب…", nick_optional: "اسم مختصر (اختياري)",
    nick_ph: "مثال: إنترنت البيت", verify: "تحقق من كلمة المرور", verifying: "جارٍ التحقق…", cancel: "إلغاء",
    verified: "تم التحقق من كلمة المرور", try_again: "إعادة المحاولة", err_title: "كلمة المرور غير صحيحة",
    rename_title: "تغيير اسم الرقم", nickname: "الاسم المختصر", nick_hint: "يغيّر طريقة ظهور الرقم في Klue فقط.",
    shown_as: "يظهر باسم", use_owner: "استخدام اسم صاحب الخط", save: "حفظ",
    quota_alerts: "تنبيهات الرصيد", renewal_reminder: "تذكير التجديد", remind_me: "ذكّرني قبل", before_renewal: "من التجديد",
    days: "أيام", hours: "ساعات", renew_day: "تنبيه يوم التجديد أيضاً", preview: "معاينة", theme: "المظهر",
    match_hint: "ألوان الشركة: يتغيّر لون التطبيق حسب الشركة عند التنقل بين التبويبات.", language: "اللغة",
    check_every: "تحديث الرصيد كل", startup: "التشغيل مع بدء النظام", tray: "العمل في الخلفية",
    disclaimer: "تطبيق غير رسمي وغير تابع لأي شركة اتصالات.", done: "تم", open_app: "فتح Klue", settings_title: "الإعدادات",
    empty_kicker: "لا توجد أرقام",
    coming_soon_body: "لا يستطيع Klue تسجيل الدخول إلى هذه الشركة بعد. يعمل حالياً إنترنت وي المنزلي وخطوط فودافون المحمول، وأورنج وإي آند في الطريق.",
    not_synced: "لم يتم التحديث بعد", syncing: "جارٍ التحديث…", synced_now: "تم التحديث الآن", waiting_first_sync: "في انتظار أول تحديث…", owner_unknown: "لم توفره الشركة", remove: "حذف الرقم",
    remove_hint: "يحذف الرقم وكلمة المرور المحفوظة من هذا الجهاز.", remove_confirm: "اضغط مرة أخرى للحذف",
    captcha_title: "اكتب الحروف", captcha_body: "تريد وي التأكد من أنك لست برنامجاً قبل تسجيل الدخول. اكتب الحروف الظاهرة في الصورة.",
    captcha_ph: "الحروف", continue_: "متابعة", already_added: "هذا الرقم مضاف بالفعل في Klue.",
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
    owed_note: "يُخصم أولًا من الشحنة القادمة.",
    we_mobile_soon: "خطوط وي المحمول (015) قريباً.",
    vf_dsl_soon: "فودافون DSL قريباً.",
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

pub fn line_name(line: LineType, lang: Lang) -> &'static str {
    let s = strings(lang);
    match line {
        LineType::Mobile => s.mobile,
        LineType::Landline => s.landline,
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
pub fn error_text(e: &FetchError, carrier: Carrier, lang: Lang) -> String {
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
            let t = until.with_timezone(&Local).format("%H:%M");
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
        FetchError::Unsupported => strings(lang).coming_soon_body.into(),
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
        FetchError::Network(_) => if ar { "لا يوجد اتصال" } else { "No connection" },
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

/// "14:07", local.
pub fn clock(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%H:%M").to_string()
}

/// "Mon 23 Nov, 14:07" / "الاثنين 23 نوفمبر، 14:07".
pub fn moment(dt: DateTime<Utc>, lang: Lang) -> String {
    match lang {
        Lang::En => format!("{}, {}", day_label(dt, lang), clock(dt)),
        Lang::Ar => format!("{}، {}", day_label(dt, lang), clock(dt)),
    }
}

/// "Mon 23 Nov, 14:00–15:00".
pub fn hour_span(start: DateTime<Utc>, end: DateTime<Utc>, lang: Lang) -> String {
    match lang {
        Lang::En => format!("{}, {}–{}", day_label(start, lang), clock(start), clock(end)),
        Lang::Ar => format!("{}، {}–{}", day_label(start, lang), clock(start), clock(end)),
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
pub fn used_since(v: f64, unit: Unit, since: DateTime<Utc>, lang: Lang) -> String {
    let q = quantity(v, unit, lang);
    if is_ar(lang) { format!("{q} منذ {}", clock(since)) } else { format!("{q} since {}", clock(since)) }
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
pub fn from_clock(t: DateTime<Utc>, lang: Lang) -> String {
    if is_ar(lang) { format!("من {}", clock(t)) } else { format!("From {}", clock(t)) }
}

pub fn until_clock(t: DateTime<Utc>, lang: Lang) -> String {
    if is_ar(lang) { format!("حتى {}", clock(t)) } else { format!("Until {}", clock(t)) }
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
