//! Authorization code lifted from `sunk`

use std::iter;

use rand::{distributions::Alphanumeric, thread_rng, Rng};

#[derive(Debug)]
pub struct SubsonicAuth {
    user: String,
    password: String,
}

const SALT_SIZE: usize = 36; // Minimum 6 characters.

impl SubsonicAuth {
    pub fn new(user: impl Into<String>, password: impl Into<String>) -> SubsonicAuth {
        SubsonicAuth {
            user: user.into(),
            password: password.into(),
        }
    }

    pub fn add_to_query_pairs(
        &self,
        query_pairs: &mut url::form_urlencoded::Serializer<url::UrlQuery>,
    ) {
        let mut rng = thread_rng();
        let salt: String = iter::repeat(())
            .map(|()| char::from(rng.sample(Alphanumeric)))
            .take(SALT_SIZE)
            .collect();

        let pre_t = self.password.to_string() + &salt;
        let token = format!("{:x}", md5::compute(pre_t.as_bytes()));

        query_pairs.append_pair("u", &self.user);
        query_pairs.append_pair("t", &token);
        query_pairs.append_pair("s", &salt);

        let format = "json";
        let crate_name = env!("CARGO_PKG_NAME");

        query_pairs.append_pair("v", "1.16.1");
        query_pairs.append_pair("c", crate_name);
        query_pairs.append_pair("f", format);
    }
}
