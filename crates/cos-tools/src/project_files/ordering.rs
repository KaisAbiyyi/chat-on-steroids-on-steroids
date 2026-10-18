use icu_collator::{CollatorBorrowed, options::CollatorOptions};
use icu_locale_core::Locale;
use std::{cmp::Ordering, io};

/// The source uses localeCompare with the process default locale and stable ties.
pub(super) fn collator() -> io::Result<CollatorBorrowed<'static>> {
    let locale = locale_name()
        .split(['.', '@'])
        .next()
        .unwrap_or("en-US")
        .replace('_', "-");
    let locale = locale
        .parse::<Locale>()
        .unwrap_or_else(|_| "en-US".parse().unwrap());
    CollatorBorrowed::try_new(locale.into(), CollatorOptions::default()).map_err(io::Error::other)
}

fn locale_name() -> String {
    #[cfg(windows)]
    {
        let mut name = [0u16; 85];
        // Windows locale names include a terminating NUL in the returned length.
        let length = unsafe {
            windows_sys::Win32::Globalization::GetUserDefaultLocaleName(
                name.as_mut_ptr(),
                name.len() as i32,
            )
        };
        if length > 1 {
            return String::from_utf16_lossy(&name[..length as usize - 1]);
        }
    }
    #[cfg(unix)]
    {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = std::env::var(key)
                && !value.is_empty()
                && !matches!(value.as_str(), "C" | "POSIX" | "C.UTF-8")
            {
                return value;
            }
        }
    }
    "en-US".into()
}

pub(super) struct Key<'a> {
    pub collator: &'a CollatorBorrowed<'static>,
    pub non_directory: bool,
    pub name: String,
    pub index: usize,
}
impl PartialEq for Key<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Key<'_> {}
impl PartialOrd for Key<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.non_directory
            .cmp(&other.non_directory)
            .then_with(|| self.collator.compare(&self.name, &other.name))
            .then_with(|| self.index.cmp(&other.index))
    }
}
