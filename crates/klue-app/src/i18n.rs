//! English and Arabic strings, from the `S` table in the mockup plus the few screens
//! the mockup doesn't cover (coming soon, captcha, sync errors).

use chrono::{DateTime, Datelike, Local, Utc};
use klue_core::{BundleKind, Carrier, FetchError, LineType, Unit, store::Lang};

pub struct Strings {
    pub app: &'static str,
    pub light: &'static str,
    pub dark: &'static str,
    pub match_: &'static str,
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
    // Not in the mockup:
    pub soon: &'static str,
    pub coming_soon: &'static str,
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
}

pub static EN: Strings = Strings {
    app: "Klue", light: "Light", dark: "Dark", match_: "Match", match_long: "Match carrier", add: "Add", settings: "Settings",
    add_number: "Add number", mobile: "Mobile", landline: "Landline", registered_to: "Registered to", refresh: "Refresh",
    rename: "Rename", alerts: "Alerts", package: "Package", below_alert: "Below alert", remaining: "remaining", change: "Change",
    next_renewal: "Next renewal", cycle: "Billing cycle", subscribed: "Subscribed", renews: "Renews", bundles: "Bundles & add-ons",
    col_bundle: "Bundle", col_rem: "Remaining", col_total: "Total", col_left: "Left", col_alert: "Alert", off: "Off",
    empty_body: "Add a number and its account password. Klue signs in, reads the quota, and fills in the registered owner for you. Mobile and landline numbers both work.",
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
    soon: "Soon", coming_soon: "Coming soon",
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
};

pub static AR: Strings = Strings {
    app: "Klue", light: "فاتح", dark: "داكن", match_: "الشركة", match_long: "ألوان الشركة", add: "إضافة", settings: "الإعدادات",
    add_number: "إضافة رقم", mobile: "محمول", landline: "أرضي", registered_to: "مسجّل باسم", refresh: "تحديث", rename: "تغيير الاسم",
    alerts: "التنبيهات", package: "الباقة", below_alert: "أقل من حد التنبيه", remaining: "متبقي", change: "تعديل",
    next_renewal: "التجديد القادم", cycle: "دورة الاشتراك", subscribed: "تاريخ الاشتراك", renews: "يتجدد", bundles: "الباقات والإضافات",
    col_bundle: "الباقة", col_rem: "المتبقي", col_total: "الإجمالي", col_left: "النسبة", col_alert: "التنبيه", off: "غير مفعّل",
    empty_body: "أضف الرقم وكلمة مرور الحساب. يسجّل Klue الدخول ويقرأ الباقة ويملأ اسم صاحب الخط تلقائياً. يدعم أرقام المحمول والأرضي.",
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
    soon: "قريباً", coming_soon: "قريباً",
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
};

pub fn strings(lang: Lang) -> &'static Strings {
    match lang {
        Lang::En => &EN,
        Lang::Ar => &AR,
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

#[derive(Clone, Copy)]
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
        Lang::En => format!("in {d} days, {h} hrs"),
        Lang::Ar => format!("بعد {d} يوم و{h} ساعة"),
    }
}

/// User-facing explanation of a failed refresh or verification.
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

/// Title for the error box in the add-number flow.
pub fn error_title(e: &FetchError, lang: Lang) -> &'static str {
    let ar = is_ar(lang);
    match e {
        FetchError::BadCredentials => strings(lang).err_title,
        FetchError::CaptchaUnsolved | FetchError::CaptchaBlocked(_) => if ar { "مطلوب رمز تحقق" } else { "Captcha needed" },
        FetchError::CaptchaRejected => if ar { "لم يُقبل رمز التحقق" } else { "Captcha not accepted" },
        FetchError::InvalidNumber(_) => if ar { "رقم غير صحيح" } else { "Check the number" },
        FetchError::Network(_) => if ar { "لا يوجد اتصال" } else { "No connection" },
        _ => if ar { "تعذّر التحقق" } else { "Couldn’t verify" },
    }
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
    fn dates_match_the_mockup_formats() {
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
    }
}
