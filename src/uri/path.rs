use std::convert::TryFrom;
use std::str::FromStr;
use std::{cmp, fmt, hash, str};

use bytes::Bytes;

use super::{ErrorKind, InvalidUri};
use crate::byte_str::ByteStr;

/// Added for backwards compatibility
#[deprecated(note = "use `PathAndQueryWithFragment` instead")]
pub type PathAndQuery = PathAndQueryWithFragment;

/// Represents the path component of a URI
#[derive(Clone)]
pub struct PathAndQueryWithFragment {
    pub(super) data: ByteStr,
    pub(super) query: u16,
    pub(super) fragment: u16,
}

const NONE: u16 = u16::MAX;

impl PathAndQueryWithFragment {
    // Not public while `bytes` is unstable.
    pub(super) fn from_shared(src: Bytes) -> Result<Self, InvalidUri> {
        let Scanned {
            query,
            fragment,
            is_maybe_not_utf8,
        } = scan_path_and_query(&src)?;

        let data = if is_maybe_not_utf8 {
            ByteStr::from_utf8(src).map_err(|_| ErrorKind::InvalidUriChar)?
        } else {
            unsafe { ByteStr::from_utf8_unchecked(src) }
        };

        Ok(PathAndQueryWithFragment { data, query, fragment })
    }

    /// Convert a `PathAndQuery` from a static string.
    ///
    /// This function will not perform any copying, however the string is
    /// checked to ensure that it is valid.
    ///
    /// # Panics
    ///
    /// This function panics if the argument is an invalid path and query.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::uri::*;
    /// let v = PathAndQueryWithFragment::from_static("/hello?world");
    ///
    /// assert_eq!(v.path(), "/hello");
    /// assert_eq!(v.query(), Some("world"));
    /// ```
    #[inline]
    pub const fn from_static(src: &'static str) -> Self {
        match scan_path_and_query(src.as_bytes()) {
            Ok(Scanned {
                query,
                fragment,
                is_maybe_not_utf8: false,
            }) => PathAndQueryWithFragment {
                data: ByteStr::from_static(src),
                query,
                fragment
            },
            // Yes, we reject non-utf8
            _ => panic!("static str is not valid path"),
        }
    }

    /// Attempt to convert a `Bytes` buffer to a `PathAndQuery`.
    ///
    /// This will try to prevent a copy if the type passed is the type used
    /// internally, and will copy the data if it is not.
    pub fn from_maybe_shared<T>(src: T) -> Result<Self, InvalidUri>
    where
        T: AsRef<[u8]> + 'static,
    {
        if_downcast_into!(T, Bytes, src, {
            return PathAndQueryWithFragment::from_shared(src);
        });

        PathAndQueryWithFragment::try_from(src.as_ref())
    }

    pub(super) fn empty() -> Self {
        PathAndQueryWithFragment {
            data: ByteStr::new(),
            query: NONE,
            fragment: NONE
        }
    }

    pub(super) fn slash() -> Self {
        PathAndQueryWithFragment {
            data: ByteStr::from_static("/"),
            query: NONE,
            fragment: NONE
        }
    }

    pub(super) fn star() -> Self {
        PathAndQueryWithFragment {
            data: ByteStr::from_static("*"),
            query: NONE,
            fragment: NONE
        }
    }

    /// Returns the path component
    ///
    /// The path component is **case sensitive**.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    ///                                        |--------|
    ///                                             |
    ///                                           path
    /// ```
    ///
    /// If the URI is `*` then the path component is equal to `*`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::uri::*;
    ///
    /// let path_and_query: PathAndQueryWithFragment = "/hello/world".parse().unwrap();
    ///
    /// assert_eq!(path_and_query.path(), "/hello/world");
    /// ```
    #[inline]
    pub fn path(&self) -> &str {
        let ret = if self.query == NONE {
            &self.data[..]
        } else {
            &self.data[..self.query as usize]
        };

        if ret.is_empty() {
            return "/";
        }

        ret
    }

    /// Returns the query string component
    ///
    /// The query component contains non-hierarchical data that, along with data
    /// in the path component, serves to identify a resource within the scope of
    /// the URI's scheme and naming authority (if any). The query component is
    /// indicated by the first question mark ("?") character and terminated by a
    /// number sign ("#") character or by the end of the URI.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    ///                                                   |-------------------|
    ///                                                             |
    ///                                                           query
    /// ```
    ///
    /// # Examples
    ///
    /// With a query string component
    ///
    /// ```
    /// # use http::uri::*;
    /// let path_and_query: PathAndQueryWithFragment = "/hello/world?key=value&foo=bar".parse().unwrap();
    ///
    /// assert_eq!(path_and_query.query(), Some("key=value&foo=bar"));
    /// ```
    ///
    /// Without a query string component
    ///
    /// ```
    /// # use http::uri::*;
    /// let path_and_query: PathAndQueryWithFragment = "/hello/world".parse().unwrap();
    ///
    /// assert!(path_and_query.query().is_none());
    /// ```
    #[inline]
    pub fn query(&self) -> Option<&str> {
        match (self.query, self.fragment) {
            (NONE, _) => { None },
            (query, NONE) => {
                let i = query + 1;
                Some(&self.data[i as usize..])
            },
            (query, fragment) => {
                let i = query + 1;
                Some(&self.data[i as usize..fragment as usize])
            }
        }
    }

    /// Get the fragment string of this `Uri`, starting after the `#`.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    ///                                                                         |-----|
    ///                                                                            |
    ///                                                                         fragment
    /// ```
    ///
    /// # Examples
    ///
    /// Simple fragment
    ///
    /// ```
    /// # use http::Uri;
    /// let uri: Uri = "http://example.org/hello/world?key=value#fragid1".parse().unwrap();
    ///
    /// assert_eq!(uri.fragment(), Some("fragid1"));
    /// ```
    #[inline]
    pub fn fragment(&self) -> Option<&str> {
        if self.fragment == NONE {
            None
        } else {
            let i = self.fragment + 1;
            Some(&self.data[i as usize..])
        }
    }

    /// Returns the path and query as a string component.
    ///
    /// # Examples
    ///
    /// With a query string component
    ///
    /// ```
    /// # use http::uri::*;
    /// let path_and_query: PathAndQueryWithFragment = "/hello/world?key=value&foo=bar".parse().unwrap();
    ///
    /// assert_eq!(path_and_query.as_str(), "/hello/world?key=value&foo=bar");
    /// ```
    ///
    /// Without a query string component
    ///
    /// ```
    /// # use http::uri::*;
    /// let path_and_query: PathAndQueryWithFragment = "/hello/world".parse().unwrap();
    ///
    /// assert_eq!(path_and_query.as_str(), "/hello/world");
    /// ```
    #[inline]
    pub fn as_str(&self) -> &str {
        let ret = &self.data[..];
        if ret.is_empty() {
            return "/";
        }
        ret
    }
}

impl TryFrom<&[u8]> for PathAndQueryWithFragment {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &[u8]) -> Result<Self, Self::Error> {
        PathAndQueryWithFragment::from_shared(Bytes::copy_from_slice(s))
    }
}

impl TryFrom<&str> for PathAndQueryWithFragment {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        TryFrom::try_from(s.as_bytes())
    }
}

impl TryFrom<Vec<u8>> for PathAndQueryWithFragment {
    type Error = InvalidUri;
    #[inline]
    fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
        PathAndQueryWithFragment::from_shared(vec.into())
    }
}

impl TryFrom<String> for PathAndQueryWithFragment {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: String) -> Result<Self, Self::Error> {
        PathAndQueryWithFragment::from_shared(s.into())
    }
}

impl TryFrom<&String> for PathAndQueryWithFragment {
    type Error = InvalidUri;
    #[inline]
    fn try_from(s: &String) -> Result<Self, Self::Error> {
        TryFrom::try_from(s.as_bytes())
    }
}

impl FromStr for PathAndQueryWithFragment {
    type Err = InvalidUri;
    #[inline]
    fn from_str(s: &str) -> Result<Self, InvalidUri> {
        TryFrom::try_from(s)
    }
}

impl fmt::Debug for PathAndQueryWithFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for PathAndQueryWithFragment {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.data.is_empty() {
            match self.data.as_bytes()[0] {
                b'/' | b'*' => write!(fmt, "{}", &self.data[..]),
                _ => write!(fmt, "/{}", &self.data[..]),
            }
        } else {
            write!(fmt, "/")
        }
    }
}

impl hash::Hash for PathAndQueryWithFragment {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}

// ===== PartialEq / PartialOrd =====

impl PartialEq for PathAndQueryWithFragment {
    #[inline]
    fn eq(&self, other: &PathAndQueryWithFragment) -> bool {
        self.data == other.data
    }
}

impl Eq for PathAndQueryWithFragment {}

impl PartialEq<str> for PathAndQueryWithFragment {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<PathAndQueryWithFragment> for &str {
    #[inline]
    fn eq(&self, other: &PathAndQueryWithFragment) -> bool {
        self == &other.as_str()
    }
}

impl PartialEq<&str> for PathAndQueryWithFragment {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<PathAndQueryWithFragment> for str {
    #[inline]
    fn eq(&self, other: &PathAndQueryWithFragment) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<String> for PathAndQueryWithFragment {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<PathAndQueryWithFragment> for String {
    #[inline]
    fn eq(&self, other: &PathAndQueryWithFragment) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialOrd for PathAndQueryWithFragment {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQueryWithFragment) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl PartialOrd<str> for PathAndQueryWithFragment {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other)
    }
}

impl PartialOrd<PathAndQueryWithFragment> for str {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQueryWithFragment) -> Option<cmp::Ordering> {
        self.partial_cmp(other.as_str())
    }
}

impl PartialOrd<&str> for PathAndQueryWithFragment {
    #[inline]
    fn partial_cmp(&self, other: &&str) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(*other)
    }
}

impl PartialOrd<PathAndQueryWithFragment> for &str {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQueryWithFragment) -> Option<cmp::Ordering> {
        self.partial_cmp(&other.as_str())
    }
}

impl PartialOrd<String> for PathAndQueryWithFragment {
    #[inline]
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl PartialOrd<PathAndQueryWithFragment> for String {
    #[inline]
    fn partial_cmp(&self, other: &PathAndQueryWithFragment) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

// Scanner implementation that is `const fn`, usable by both `from_static`
// and `from_shared`.
// =====

struct Scanned {
    query: u16,
    fragment: u16,
    is_maybe_not_utf8: bool,
}

const fn scan_path_and_query(bytes: &[u8]) -> Result<Scanned, ErrorKind> {
    let mut i = 0;
    let mut query = NONE;
    let mut fragment = NONE;

    let mut is_maybe_not_utf8 = false;

    if bytes.is_empty() {
        return Err(ErrorKind::Empty);
    }

    if bytes.len() == 1 && bytes[0] == b'*' {
        return Ok(Scanned {
            query,
            fragment,
            is_maybe_not_utf8: false,
        });
    }

    if !matches!(bytes[0], b'/' | b'?' | b'#') {
        return Err(ErrorKind::PathDoesNotStartWithSlash);
    }

    while i < bytes.len() {
        // See https://url.spec.whatwg.org/#path-state
        match bytes[i] {
            b'?' => {
                debug_assert!(query == NONE);
                query = i as u16;
                i += 1;
                break;
            }
            b'#' => {
                fragment = i as u16;
                break;
            }

            // This is the range of bytes that don't need to be
            // percent-encoded in the path. If it should have been
            // percent-encoded, then error.
            #[rustfmt::skip]
            0x21 |
            0x24..=0x3B |
            0x3D |
            0x40..=0x5F |
            0x61..=0x7A |
            0x7C |
            0x7E => {}

            // potentially utf8, might not, should check
            0x80..=0xFF => {
                is_maybe_not_utf8 = true;
            }

            // These are code points that are supposed to be
            // percent-encoded in the path but there are clients
            // out there sending them as is and httparse accepts
            // to parse those requests, so they are allowed here
            // for parity.
            //
            // For reference, those are code points that are used
            // to send requests with JSON directly embedded in
            // the URI path. Yes, those things happen for real.
            #[rustfmt::skip]
            b'"' |
            b'{' | b'}' => {}

            _ => return Err(ErrorKind::InvalidUriChar),
        }
        i += 1;
    }

    // query ...
    if query != NONE {
        while i < bytes.len() {
            match bytes[i] {
                // While queries *should* be percent-encoded, most
                // bytes are actually allowed...
                // See https://url.spec.whatwg.org/#query-state
                //
                // Allowed: 0x21 / 0x24 - 0x3B / 0x3D / 0x3F - 0x7E
                #[rustfmt::skip]
                0x21 |
                0x24..=0x3B |
                0x3D |
                0x3F..=0x7E => {}

                0x80..=0xFF => {
                    is_maybe_not_utf8 = true;
                }

                b'#' => {
                    fragment = i as u16;
                    break;
                }

                _ => return Err(ErrorKind::InvalidUriChar),
            }
            i += 1;
        }
    }

    Ok(Scanned {
        query,
        fragment,
        is_maybe_not_utf8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_to_self_of_same_path() {
        let p1: PathAndQueryWithFragment = "/hello/world&foo=bar".parse().unwrap();
        let p2: PathAndQueryWithFragment = "/hello/world&foo=bar".parse().unwrap();
        assert_eq!(p1, p2);
        assert_eq!(p2, p1);
    }

    #[test]
    fn not_equal_to_self_of_different_path() {
        let p1: PathAndQueryWithFragment = "/hello/world&foo=bar".parse().unwrap();
        let p2: PathAndQueryWithFragment = "/world&foo=bar".parse().unwrap();
        assert_ne!(p1, p2);
        assert_ne!(p2, p1);
    }

    #[test]
    fn equates_with_a_str() {
        let path_and_query: PathAndQueryWithFragment = "/hello/world&foo=bar".parse().unwrap();
        assert_eq!(&path_and_query, "/hello/world&foo=bar");
        assert_eq!("/hello/world&foo=bar", &path_and_query);
        assert_eq!(path_and_query, "/hello/world&foo=bar");
        assert_eq!("/hello/world&foo=bar", path_and_query);
    }

    #[test]
    fn not_equal_with_a_str_of_a_different_path() {
        let path_and_query: PathAndQueryWithFragment = "/hello/world&foo=bar".parse().unwrap();
        // as a reference
        assert_ne!(&path_and_query, "/hello&foo=bar");
        assert_ne!("/hello&foo=bar", &path_and_query);
        // without reference
        assert_ne!(path_and_query, "/hello&foo=bar");
        assert_ne!("/hello&foo=bar", path_and_query);
    }

    #[test]
    fn equates_with_a_string() {
        let path_and_query: PathAndQueryWithFragment = "/hello/world&foo=bar".parse().unwrap();
        assert_eq!(path_and_query, "/hello/world&foo=bar".to_string());
        assert_eq!("/hello/world&foo=bar".to_string(), path_and_query);
    }

    #[test]
    fn not_equal_with_a_string_of_a_different_path() {
        let path_and_query: PathAndQueryWithFragment = "/hello/world&foo=bar".parse().unwrap();
        assert_ne!(path_and_query, "/hello&foo=bar".to_string());
        assert_ne!("/hello&foo=bar".to_string(), path_and_query);
    }

    #[test]
    fn compares_to_self() {
        let p1: PathAndQueryWithFragment = "/a/world&foo=bar".parse().unwrap();
        let p2: PathAndQueryWithFragment = "/b/world&foo=bar".parse().unwrap();
        assert!(p1 < p2);
        assert!(p2 > p1);
    }

    #[test]
    fn compares_with_a_str() {
        let path_and_query: PathAndQueryWithFragment = "/b/world&foo=bar".parse().unwrap();
        // by ref
        assert!(&path_and_query < "/c/world&foo=bar");
        assert!("/c/world&foo=bar" > &path_and_query);
        assert!(&path_and_query > "/a/world&foo=bar");
        assert!("/a/world&foo=bar" < &path_and_query);

        // by val
        assert!(path_and_query < "/c/world&foo=bar");
        assert!("/c/world&foo=bar" > path_and_query);
        assert!(path_and_query > "/a/world&foo=bar");
        assert!("/a/world&foo=bar" < path_and_query);
    }

    #[test]
    fn compares_with_a_string() {
        let path_and_query: PathAndQueryWithFragment = "/b/world&foo=bar".parse().unwrap();
        assert!(path_and_query < "/c/world&foo=bar".to_string());
        assert!("/c/world&foo=bar".to_string() > path_and_query);
        assert!(path_and_query > "/a/world&foo=bar".to_string());
        assert!("/a/world&foo=bar".to_string() < path_and_query);
    }

    #[test]
    fn ignores_valid_percent_encodings() {
        assert_eq!("/a%20b", pqf("/a%20b?r=1").path());
        assert_eq!("qr=%31", pqf("/a/b?qr=%31").query().unwrap());
    }

    #[test]
    fn ignores_invalid_percent_encodings() {
        assert_eq!("/a%%b", pqf("/a%%b?r=1").path());
        assert_eq!("/aaa%", pqf("/aaa%").path());
        assert_eq!("/aaa%", pqf("/aaa%?r=1").path());
        assert_eq!("/aa%2", pqf("/aa%2").path());
        assert_eq!("/aa%2", pqf("/aa%2?r=1").path());
        assert_eq!("qr=%3", pqf("/a/b?qr=%3").query().unwrap());
    }

    #[test]
    fn allow_utf8_in_path() {
        assert_eq!("/🍕", pqf("/🍕").path());
    }

    #[test]
    fn allow_utf8_in_query() {
        assert_eq!(Some("pizza=🍕"), pqf("/test?pizza=🍕").query());
    }

    #[test]
    fn rejects_invalid_utf8_in_path() {
        PathAndQueryWithFragment::try_from(&[b'/', 0xFF][..]).expect_err("reject invalid utf8");
    }

    #[test]
    fn rejects_invalid_utf8_in_query() {
        PathAndQueryWithFragment::try_from(&[b'/', b'a', b'?', 0xFF][..]).expect_err("reject invalid utf8");
    }

    #[test]
    fn rejects_empty_string() {
        PathAndQueryWithFragment::try_from("").expect_err("reject empty str");
    }

    #[test]
    fn requires_starting_with_slash() {
        PathAndQueryWithFragment::try_from("sneaky").expect_err("reject missing slash");
    }

    #[test]
    fn rejects_del_in_path() {
        PathAndQueryWithFragment::try_from(&[b'/', 0x7F][..]).expect_err("reject DEL");
    }

    #[test]
    fn rejects_del_in_query() {
        PathAndQueryWithFragment::try_from(&[b'/', b'a', b'?', 0x7F][..]).expect_err("reject DEL");
    }

    #[test]
    fn json_is_fine() {
        assert_eq!(
            r#"/{"bread":"baguette"}"#,
            pqf(r#"/{"bread":"baguette"}"#).path()
        );
    }

    #[test]
    fn test_fragment_parsing() {
        let url = pqf("/page?search=cats#result-4");
        assert_eq!(url.query(), Some("search=cats"));
        assert_eq!(url.fragment(), Some("result-4"));

        let url_no_query = pqf("/page#about");
        assert_eq!(url_no_query.query(), None);
        assert_eq!(url_no_query.fragment(), Some("about"));

        let url_no_fragment = pqf("/page?search=cats");
        assert_eq!(url_no_fragment.query(), Some("search=cats"));
        assert_eq!(url_no_fragment.fragment(), None);

        let url_pure_path = pqf("/page");
        assert_eq!(url_pure_path.query(), None);
        assert_eq!(url_pure_path.fragment(), None);
    }

    fn pqf(s: &str) -> PathAndQueryWithFragment {
        s.parse().expect(&format!("parsing {}", s))
    }
}
