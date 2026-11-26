//! The kinds of line each carrier sells, as the Add wizard lists them. Only some can be read
//! yet; the others show as coming.

use klue_core::{Carrier, LineType};

use crate::assets::Icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Product {
    WeHomeInternet,
    /// Home internet and an Ardy landline on one account.
    WeOneHome,
    WeArdy,
    /// Mobile minutes and internet.
    WeMobile,
    /// 4G home internet.
    WeAir,
    /// Mobile minutes, home internet or WE Air.
    WeGold,
    VfMobile,
    VfHomeInternet,
    /// USB modems and mobile Wi-Fi routers.
    VfUsb,
    OrMobile,
    OrHomeInternet,
    EaMobile,
    EaHomeInternet,
}

impl Product {
    /// A carrier's kinds of line, the ones Klue reads first.
    pub fn of(carrier: Carrier) -> &'static [Product] {
        use Product::*;
        match carrier {
            Carrier::We => &[WeHomeInternet, WeOneHome, WeArdy, WeMobile, WeAir, WeGold],
            Carrier::Vodafone => &[VfMobile, VfHomeInternet, VfUsb],
            Carrier::Orange => &[OrMobile, OrHomeInternet],
            Carrier::Etisalat => &[EaMobile, EaHomeInternet],
        }
    }

    /// The line Klue reads it as; `None` while it's coming.
    pub fn line(self) -> Option<LineType> {
        match self {
            Product::WeHomeInternet | Product::OrHomeInternet => Some(LineType::Landline),
            Product::VfMobile | Product::OrMobile => Some(LineType::Mobile),
            _ => None,
        }
    }

    /// What a saved line is.
    pub fn for_line(carrier: Carrier, line: LineType) -> Option<Product> {
        Product::of(carrier).iter().copied().find(|p| p.line() == Some(line))
    }

    pub fn icon(self) -> Icon {
        use Product::*;
        match self {
            WeHomeInternet | VfHomeInternet | OrHomeInternet | EaHomeInternet => Icon::Router,
            WeOneHome => Icon::House,
            WeArdy => Icon::Landline,
            WeMobile | VfMobile | OrMobile | EaMobile => Icon::Mobile,
            WeAir => Icon::Wifi,
            WeGold => Icon::Crown,
            VfUsb => Icon::Usb,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_line_klue_reads_has_its_product() {
        for carrier in Carrier::ALL {
            for p in Product::of(carrier) {
                assert!(Carrier::ALL.iter().filter(|c| Product::of(**c).contains(p)).eq([&carrier]), "{p:?} is listed once");
                if let Some(line) = p.line() {
                    assert!(carrier.supports_line(line), "{p:?}");
                }
            }
            for line in [LineType::Mobile, LineType::Landline] {
                assert_eq!(Product::for_line(carrier, line).is_some(), carrier.supports_line(line), "{carrier:?} {line:?}");
            }
        }
    }
}
