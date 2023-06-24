use crate::ansi::RESET;
use crate::difference::Difference;
use crate::style::{Color, Style};
use crate::write::AnyWrite;
use std::borrow::Cow;
use std::fmt;
use std::io;

#[derive(Eq, PartialEq, Debug)]
enum OSControl<'a, S: 'a + ToOwned + ?Sized>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    Title,
    Link { url: Cow<'a, S> },
}

impl<'a, S: 'a + ToOwned + ?Sized> Clone for OSControl<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    fn clone(&self) -> Self {
        match self {
            Self::Link { url: u } => Self::Link { url: u.clone() },
            Self::Title => Self::Title,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum Wrapping<'a> {
    CtrlACtrlB,
    Str(&'a str, &'a str),
}

/// An `AnsiGenericString` includes a generic string type and a `Style` to
/// display that string.  `AnsiString` and `AnsiByteString` are aliases for
/// this type on `str` and `\[u8]`, respectively.
#[derive(Eq, PartialEq, Debug)]
pub struct AnsiGenericString<'a, S: 'a + ToOwned + ?Sized>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    pub(crate) style: Style,
    pub(crate) string: Cow<'a, S>,
    oscontrol: Option<OSControl<'a, S>>,
    // whether/what to wrap zero-width parts with
    pub wrap_zw: Option<Wrapping<'a>>,
}

/// Cloning an `AnsiGenericString` will clone its underlying string.
///
/// # Examples
///
/// ```
/// use nu_ansi_term::AnsiString;
///
/// let plain_string = AnsiString::from("a plain string");
/// let clone_string = plain_string.clone();
/// assert_eq!(clone_string, plain_string);
/// ```
impl<'a, S: 'a + ToOwned + ?Sized> Clone for AnsiGenericString<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    fn clone(&self) -> AnsiGenericString<'a, S> {
        AnsiGenericString {
            style: self.style,
            string: self.string.clone(),
            oscontrol: self.oscontrol.clone(),
            wrap_zw: self.wrap_zw.clone(),
        }
    }
}

// You might think that the hand-written Clone impl above is the same as the
// one that gets generated with #[derive]. But it’s not *quite* the same!
//
// `str` is not Clone, and the derived Clone implementation puts a Clone
// constraint on the S type parameter (generated using --pretty=expanded):
//
//                  ↓_________________↓
//     impl <'a, S: ::std::clone::Clone + 'a + ToOwned + ?Sized> ::std::clone::Clone
//     for ANSIGenericString<'a, S> where
//     <S as ToOwned>::Owned: fmt::Debug { ... }
//
// This resulted in compile errors when you tried to derive Clone on a type
// that used it:
//
//     #[derive(PartialEq, Debug, Clone, Default)]
//     pub struct TextCellContents(Vec<AnsiString<'static>>);
//                                 ^^^^^^^^^^^^^^^^^^^^^^^^^
//     error[E0277]: the trait `std::clone::Clone` is not implemented for `str`
//
// The hand-written impl above can ignore that constraint and still compile.

/// An ANSI String is a string coupled with the `Style` to display it
/// in a terminal.
///
/// Although not technically a string itself, it can be turned into
/// one with the `to_string` method.
///
/// # Examples
///
/// ```
/// use nu_ansi_term::AnsiString;
/// use nu_ansi_term::Color::Red;
///
/// let red_string = Red.paint("a red string");
/// println!("{}", red_string);
/// ```
///
/// ```
/// use nu_ansi_term::AnsiString;
///
/// let plain_string = AnsiString::from("a plain string");
/// ```
pub type AnsiString<'a> = AnsiGenericString<'a, str>;

/// An `AnsiByteString` represents a formatted series of bytes.  Use
/// `AnsiByteString` when styling text with an unknown encoding.
pub type AnsiByteString<'a> = AnsiGenericString<'a, [u8]>;

impl<'a, I, S: 'a + ToOwned + ?Sized> From<I> for AnsiGenericString<'a, S>
where
    I: Into<Cow<'a, S>>,
    <S as ToOwned>::Owned: fmt::Debug,
{
    fn from(input: I) -> AnsiGenericString<'a, S> {
        AnsiGenericString {
            string: input.into(),
            style: Style::default(),
            oscontrol: None,
            wrap_zw: None,
        }
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> AnsiGenericString<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
{
    /// Directly access the style
    pub const fn style_ref(&self) -> &Style {
        &self.style
    }

    /// Directly access the style mutably
    pub fn style_ref_mut(&mut self) -> &mut Style {
        &mut self.style
    }

    /// Directly access the underlying string
    pub fn as_str(&self) -> &S {
        self.string.as_ref()
    }

    // Instances that imply wrapping in OSC sequences
    // and do not get displayed in the terminal text
    // area.
    //
    /// Produce an ANSI string that changes the title shown
    /// by the terminal emulator.
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::AnsiGenericString;
    /// let title_string = AnsiGenericString::title("My Title");
    /// println!("{}", title_string);
    /// ```
    /// Should produce an empty line but set the terminal title.
    pub fn title<I>(s: I) -> Self
    where
        I: Into<Cow<'a, S>>,
    {
        Self {
            style: Style::default(),
            string: s.into(),
            oscontrol: Some(OSControl::<'a, S>::Title),
            wrap_zw: None,
        }
    }

    //
    // Annotations (OSC sequences that do more than wrap)
    //

    /// Cause the styled ANSI string to link to the given URL
    ///
    /// # Examples
    ///
    /// ```
    /// use nu_ansi_term::Color::Red;
    ///
    /// let mut link_string = Red.paint("a red string");
    /// link_string.hyperlink("https://www.example.com");
    /// println!("{}", link_string);
    /// ```
    /// Should show a red-painted string which, on terminals
    /// that support it, is a clickable hyperlink.
    pub fn hyperlink<I>(&mut self, url: I)
    where
        I: Into<Cow<'a, S>>,
    {
        self.oscontrol = Some(OSControl::Link { url: url.into() });
    }

    /// Get any URL associated with the string
    pub fn url_string(&self) -> Option<&S> {
        match &self.oscontrol {
            Some(OSControl::Link { url: u }) => Some(u.as_ref()),
            _ => None,
        }
    }
}

/// A set of `AnsiGenericStrings`s collected together, in order to be
/// written with a minimum of control characters.
#[derive(Debug, Eq, PartialEq)]
pub struct AnsiGenericStrings<'a, S: 'a + ToOwned + ?Sized>(pub &'a [AnsiGenericString<'a, S>])
where
    <S as ToOwned>::Owned: fmt::Debug,
    S: PartialEq;

/// A set of `AnsiString`s collected together, in order to be written with a
/// minimum of control characters.
pub type AnsiStrings<'a> = AnsiGenericStrings<'a, str>;

/// A function to construct an `AnsiStrings` instance.
#[allow(non_snake_case)]
pub const fn AnsiStrings<'a>(arg: &'a [AnsiString<'a>]) -> AnsiStrings<'a> {
    AnsiGenericStrings(arg)
}

/// A set of `AnsiByteString`s collected together, in order to be
/// written with a minimum of control characters.
pub type AnsiByteStrings<'a> = AnsiGenericStrings<'a, [u8]>;

/// A function to construct an `AnsiByteStrings` instance.
#[allow(non_snake_case)]
pub const fn AnsiByteStrings<'a>(arg: &'a [AnsiByteString<'a>]) -> AnsiByteStrings<'a> {
    AnsiGenericStrings(arg)
}

// ---- paint functions ----

impl Style {
    /// Paints the given text with this color, returning an ANSI string.
    #[must_use]
    pub fn paint<'a, I, S: 'a + ToOwned + ?Sized>(self, input: I) -> AnsiGenericString<'a, S>
    where
        I: Into<Cow<'a, S>>,
        <S as ToOwned>::Owned: fmt::Debug,
    {
        AnsiGenericString {
            string: input.into(),
            style: self,
            oscontrol: None,
            wrap_zw: None,
        }
    }
}

impl Color {
    /// Paints the given text with this color, returning an ANSI string.
    /// This is a short-cut so you don’t have to use `Blue.normal()` just
    /// to get blue text.
    ///
    /// ```
    /// use nu_ansi_term::Color::Blue;
    /// println!("{}", Blue.paint("da ba dee"));
    /// ```
    #[must_use]
    pub fn paint<'a, I, S: 'a + ToOwned + ?Sized>(self, input: I) -> AnsiGenericString<'a, S>
    where
        I: Into<Cow<'a, S>>,
        <S as ToOwned>::Owned: fmt::Debug,
    {
        AnsiGenericString {
            string: input.into(),
            style: self.normal(),
            oscontrol: None,
            wrap_zw: None,
        }
    }
}

// ---- writers for individual ANSI strings ----

impl<'a> fmt::Display for AnsiString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let w: &mut dyn fmt::Write = f;
        self.write_to_any(w)
    }
}

impl<'a> AnsiByteString<'a> {
    /// Write an `AnsiByteString` to an `io::Write`.  This writes the escape
    /// sequences for the associated `Style` around the bytes.
    pub fn write_to<W: io::Write>(&self, w: &mut W) -> io::Result<()> {
        let w: &mut dyn io::Write = w;
        self.write_to_any(w)
    }
}

impl<'a, S: 'a + ToOwned + ?Sized> AnsiGenericString<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
    &'a S: AsRef<[u8]>,
{
    // write the part within the styling prefix and suffix
    fn write_inner<W: AnyWrite<Wstr = S> + ?Sized>(
        &self,
        w: &mut W,
        in_zw: &mut bool,
        wrap_zw_continues: bool,
    ) -> Result<(), W::Error> {
        let zwbegin: &str;
        let zwend: &str;
        match self.wrap_zw {
            Some(Wrapping::CtrlACtrlB) => {
                zwbegin = "\x01";
                zwend = "\x02";
            }
            Some(Wrapping::Str(begins, ends)) => {
                zwbegin = begins;
                zwend = ends;
            }
            None => {
                zwbegin = "";
                zwend = "";
            }
        }

        macro_rules! OSC {
            ($code:literal) => {
                if !*in_zw && !self.wrap_zw.is_some() {
                    write!(w, "{}\x1B]{};", zwbegin, $code)?;
                    *in_zw = true;
                } else {
                    write!(w, "\x1B]{};", $code)?;
                }
            }
        }

        // Emit OSC String Terminator
        macro_rules! OSC_ST {
            () => {
                if *in_zw && !wrap_zw_continues {
                    *in_zw = false;
                    write!(w, "\x1B\x5C{}", zwend)
                } else {
                    write!(w, "\x1B\x5C")
                }
            }
        }

        match &self.oscontrol {
            Some(OSControl::Link { url: u }) => {
                OSC!("8;");
                w.write_str(u.as_ref())?;
                if self.wrap_zw.is_some() {
                    write!(w, "\x1B\x5C{}", zwend)?;
                    *in_zw = false;
                } else {
                    write!(w, "\x1B\x5C")?;
                }
                w.write_str(self.string.as_ref())?;
                OSC!("8;");
                OSC_ST!()
            }
            Some(OSControl::Title) => {
                OSC!("2");
                w.write_str(self.string.as_ref())?;
                OSC_ST!()
            }
            None => {
                if *in_zw {
                    write!(w, "{}", zwend)?;
                    *in_zw = false;
                }
                w.write_str(self.string.as_ref())
            }
        }
    }

    fn write_to_any<W: AnyWrite<Wstr = S> + ?Sized>(&self, w: &mut W) -> Result<(), W::Error> {
        let zwbegin: &str;
        let zwend: &str;
        match self.wrap_zw {
            Some(Wrapping::CtrlACtrlB) => {
                zwbegin = "\x01";
                zwend = "\x02";
            }
            Some(Wrapping::Str(begins, ends)) => {
                zwbegin = &begins;
                zwend = &ends;
            }
            None => {
                zwbegin = &"";
                zwend = &"";
            }
        }
        let mut in_zw: bool = true;
        write!(w, "{}{}", zwbegin, self.style.prefix())?;
        self.write_inner(w, &mut in_zw, self.wrap_zw.is_some())?;
        write!(w, "{}{}", self.style.suffix(), zwend)
    }
}

// ---- writers for combined ANSI strings ----

impl<'a> fmt::Display for AnsiStrings<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let f: &mut dyn fmt::Write = f;
        self.write_to_any(f)
    }
}

impl<'a> AnsiByteStrings<'a> {
    /// Write `AnsiByteStrings` to an `io::Write`.  This writes the minimal
    /// escape sequences for the associated `Style`s around each set of
    /// bytes.
    pub fn write_to<W: io::Write>(&self, w: &mut W) -> io::Result<()> {
        let w: &mut dyn io::Write = w;
        self.write_to_any(w)
    }
}

impl<'a, S: 'a + ToOwned + ?Sized + PartialEq> AnsiGenericStrings<'a, S>
where
    <S as ToOwned>::Owned: fmt::Debug,
    &'a S: AsRef<[u8]>,
{
    fn write_to_any<W: AnyWrite<Wstr = S> + ?Sized>(&self, w: &mut W) -> Result<(), W::Error> {
        use self::Difference::*;
        let mut zwbegin: &str;
        let mut zwend: &str;

        let first = match self.0.first() {
            None => return Ok(()),
            Some(f) => f,
        };

        match first.wrap_zw {
            Some(Wrapping::CtrlACtrlB) => {
                zwbegin = "\x01";
                zwend = "\x02";
            }
            Some(Wrapping::Str(begins, ends)) => {
                zwbegin = &begins;
                zwend = &ends;
            }
            None => {
                zwbegin = &"";
                zwend = &"";
            }
        }
        let mut styling = !first.style.is_plain();

        let mut in_zw = false; // in zero-width and wrap_zw was set
        let mut wrap_zw_continues = first.wrap_zw.is_some()
            && match self.0.get(1) {
                None => false,
                Some(second) => second.wrap_zw.is_some(),
            };

        if first.wrap_zw.is_some() {
            write!(w, "{}{}", first.style.prefix(), zwbegin)?;
            in_zw = true;
        } else {
            write!(w, "{}", first.style.prefix())?;
        }
        first.write_inner(w, &mut in_zw, wrap_zw_continues)?;

        for window in self.0.windows(2) {
            wrap_zw_continues = window[0].wrap_zw.is_some() && window[1].wrap_zw.is_some();
            styling |= !window[1].style.is_plain();
            match window[1].wrap_zw {
                Some(Wrapping::CtrlACtrlB) => {
                    zwbegin = "\x01";
                    zwend = "\x02";
                }
                Some(Wrapping::Str(begins, ends)) => {
                    zwbegin = &begins;
                    zwend = &ends;
                }
                None => {
                    zwbegin = &"";
                    zwend = &"";
                }
            }
            match Difference::between(&window[0].style, &window[1].style) {
                ExtraStyles(style) => {
                    if !in_zw {
                        in_zw = true;
                        write!(w, "{}{}", style.prefix(), zwbegin)?
                    } else {
                        write!(w, "{}", style.prefix())?
                    }
                }
                Reset => {
                    styling = false;
                    write!(w, "{}{}", RESET, window[1].style.prefix())?
                }
                Empty => { /* Do nothing! */ }
            }

            window[1].write_inner(w, &mut in_zw, wrap_zw_continues)?;
        }

        // Write the final reset string after all of the AnsiStrings have been
        // written, *except* if the last one has no styles, because it would
        // have already been written by this point.
        if let Some(last) = self.0.last() {
            if styling || !last.style.is_plain() {
                if in_zw {
                    write!(w, "{}{}", RESET, zwend)?;
                } else {
                    if last.wrap_zw.is_some() {
                        write!(w, "{}{}{}", zwbegin, RESET, zwend)?;
                    } else {
                        write!(w, "{}", RESET)?;
                    }
                }
             }
        }

        Ok(())
    }
}

// ---- tests ----

#[cfg(test)]
mod tests {
    pub use super::super::{AnsiGenericString, AnsiStrings, Wrapping};
    pub use crate::style::Color::*;
    pub use crate::style::Style;

    #[test]
    fn no_control_codes_for_plain() {
        let one = Style::default().paint("one");
        let two = Style::default().paint("two");
        let output = AnsiStrings(&[one, two]).to_string();
        assert_eq!(output, "onetwo");
    }

    // NOTE: unstyled because it could have OSC escape sequences
    fn idempotent(unstyled: AnsiGenericString<'_, str>) {
        let before_g = Green.paint("Before is Green. ");
        let before = Style::default().paint("Before is Plain. ");
        let after_g = Green.paint(" After is Green.");
        let after = Style::default().paint(" After is Plain.");
        let unstyled_s = unstyled.clone().to_string();

        // check that RESET precedes unstyled
        let joined = AnsiStrings(&[before_g.clone(), unstyled.clone()]).to_string();
        assert!(joined.starts_with("\x1B[32mBefore is Green. \x1B[0m"));
        assert!(
            joined.ends_with(unstyled_s.as_str()),
            "{:?} does not end with {:?}",
            joined,
            unstyled_s
        );

        // check that RESET does not follow unstyled when appending styled
        let joined = AnsiStrings(&[unstyled.clone(), after_g.clone()]).to_string();
        assert!(
            joined.starts_with(unstyled_s.as_str()),
            "{:?} does not start with {:?}",
            joined,
            unstyled_s
        );
        assert!(joined.ends_with("\x1B[32m After is Green.\x1B[0m"));

        // does not introduce spurious SGR codes (reset or otherwise) adjacent
        // to plain strings
        let joined = AnsiStrings(&[unstyled.clone()]).to_string();
        assert!(
            !joined.contains("\x1B["),
            "{:?} does contain \\x1B[",
            joined
        );
        let joined = AnsiStrings(&[before.clone(), unstyled.clone()]).to_string();
        assert!(
            !joined.contains("\x1B["),
            "{:?} does contain \\x1B[",
            joined
        );
        let joined = AnsiStrings(&[before.clone(), unstyled.clone(), after.clone()]).to_string();
        assert!(
            !joined.contains("\x1B["),
            "{:?} does contain \\x1B[",
            joined
        );
        let joined = AnsiStrings(&[unstyled.clone(), after.clone()]).to_string();
        assert!(
            !joined.contains("\x1B["),
            "{:?} does contain \\x1B[",
            joined
        );
    }

    #[test]
    fn title() {
        let mut title = AnsiGenericString::title("Test Title");
        assert_eq!(title.clone().to_string(), "\x1B]2;Test Title\x1B\\");
        title.wrap_zw = Some(Wrapping::CtrlACtrlB);
        assert_eq!(title.clone().to_string(), "\x01\x1B]2;Test Title\x1B\\\x02");
        idempotent(title)
    }

    #[test]
    fn hyperlink() {
        let mut styled = Red.paint("Link to example.com.");
        styled.wrap_zw = Some(Wrapping::CtrlACtrlB);
        styled.hyperlink("https://example.com");
        assert_eq!(
            styled.to_string(),
            "\x1B[31m\x01\x1B]8;;https://example.com\x1B\\\x02Link to example.com.\x01\x1B]8;;\x1B\\\x02\x1B[0m"
        );
    }

    #[test]
    fn hyperlinks() {
        let before = Green.paint("Before link. ");
        let mut link = Blue.underline().paint("Link to example.com.");
        let after = Green.paint(" After link.");
        link.wrap_zw = Some(Wrapping::CtrlACtrlB);
        link.hyperlink("https://example.com");

        // Assemble with link by itself
        let joined = AnsiStrings(&[link.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[04;34m\x01\x1B]8;;https://example.com\x1B\\\x02Link to example.com.\x01\x1B]8;;\x1B\\\x02\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[4;34m\x01\x1B]8;;https://example.com\x1B\\\x02Link to example.com.\x01\x1B]8;;\x1B\\\x02\x1B[0m"));

        // Assemble with link in the middle
        let joined = AnsiStrings(&[before.clone(), link.clone(), after.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[04;34m\x01\x1B]8;;https://example.com\x1B\\\x02Link to example.com.\x01\x1B]8;;\x1B\\\x02\x1B[0m\x1B[32m After link.\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[4;34m\x01\x1B]8;;https://example.com\x1B\\\x02Link to example.com.\x01\x1B]8;;\x1B\\\x02\x1B[0m\x1B[32m After link.\x1B[0m"));

        // Assemble with link first
        let joined = AnsiStrings(&[link.clone(), after.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[04;34m\x01\x1B]8;;https://example.com\x1B\\\x02Link to example.com.\x01\x1B]8;;\x1B\\\x02\x1B[0m\x1B[32m After link.\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[4;34m\x01\x1B]8;;https://example.com\x1B\\\x02Link to example.com.\x01\x1B]8;;\x1B\\\x02\x1B[0m\x1B[32m After link.\x1B[0m"));

        // Assemble with link at the end
        let joined = AnsiStrings(&[before.clone(), link.clone()]).to_string();
        #[cfg(feature = "gnu_legacy")]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[04;34m\x01\x1B]8;;https://example.com\x1B\\\x02Link to example.com.\x01\x1B]8;;\x1B\\\x02\x1B[0m"));
        #[cfg(not(feature = "gnu_legacy"))]
        assert_eq!(joined, format!("\x1B[32mBefore link. \x1B[4;34m\x01\x1B]8;;https://example.com\x1B\\\x02Link to example.com.\x01\x1B]8;;\x1B\\\x02\x1B[0m"));
    }
}
