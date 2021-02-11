extern crate base64;
extern crate bytes;
extern crate jsonwebtokens;
extern crate lodepng;
extern crate rgb;
extern crate rustler;
extern crate serde_json;
extern crate sha2;

use std::primitive;

use jsonwebtokens::{Algorithm, AlgorithmID, Verifier};
use rustler::{Binary, Encoder, Env, ListIterator, OwnedBinary, Term};
use rustler::atoms;
use rustler::types::atom::{false_, true_};
use rustler::types::tuple::make_tuple;
use serde_json::Value;
use sha2::{Digest, Sha256};

const STEVE_GEOMETRY: &primitive::str = "ewogICAiZ2VvbWV0cnkiIDogewogICAgICAiZGVmYXVsdCIgOiAiZ2VvbWV0cnkuaHVtYW5vaWQuY3VzdG9tIgogICB9Cn0K";
const ALEX_GEOMETRY: &primitive::str = "ewogICAiZ2VvbWV0cnkiIDogewogICAgICAiZGVmYXVsdCIgOiAiZ2VvbWV0cnkuaHVtYW5vaWQuY3VzdG9tQWxleCIKICAgfQp9Cg==";

atoms! {
    invalid_chain_data,
    invalid_client_data,
    invalid_size,
    invalid_image,
    invalid_geometry,
    hash_doesnt_match
}

#[rustler::nif]
pub fn validate_and_get_png<'a>(env: Env<'a>, chain_data: Term, client_data: &primitive::str) -> Term<'a> {
    let list_iterator: ListIterator = chain_data.decode().unwrap();

    let mojang_key: Algorithm = create_key("MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAE8ELkixyLcwlZryUQcu1TvPOmI2B7vX83ndnWRUaXm74wFfa5f/lwQNTfrLVHa2PmenpGI6JhIMUJaWZrjmMj90NoKNFSNBuKdm8rYiXsfaz3K36x/1U26HpG0ZxK/V1V");
    let verifier = Verifier::create().build().unwrap();

    let mut last_data = Value::Null;
    let mut current_key = mojang_key;

    for x in list_iterator {
        let data: &primitive::str = x.decode::<&primitive::str>().unwrap();
        let claims = verifier.verify(data, &current_key);
        if claims.is_ok() {
            last_data = claims.unwrap();
            current_key = create_key(last_data["identityPublicKey"].as_str().unwrap())
        } else if last_data != Value::Null {
            return invalid_chain_data().to_term(env);
        }
    }

    if last_data == Value::Null {
        return invalid_chain_data().to_term(env);
    }

    let claims = verifier.verify(client_data, &current_key);

    if claims.is_err() {
        return invalid_client_data().to_term(env);
    }

    let client_claims = claims.unwrap();

    let skin_width = client_claims["SkinImageWidth"].as_u64().unwrap() as usize;
    let skin_height = client_claims["SkinImageHeight"].as_u64().unwrap() as usize;

    if skin_width != 64 || (skin_height != 64 && skin_height != 32) {
        return invalid_size().to_term(env);
    }

    let skin_data = client_claims["SkinData"].as_str().unwrap();
    let raw_skin_data = base64::decode(skin_data).unwrap();

    // we have to clone, you can't use stuff for calculations and re-use it after that :/
    if raw_skin_data.len() != skin_width.clone() * skin_height.clone() * 4 {
        return invalid_size().to_term(env);
    }

    let skin_geometry_option = client_claims["SkinResourcePatch"].as_str();

    if skin_geometry_option.is_none() {
        return invalid_geometry().to_term(env);
    }

    let skin_geometry = skin_geometry_option.unwrap();

    let mut is_steve = false_();
    if skin_geometry == STEVE_GEOMETRY {
        is_steve = true_();
    } else if skin_geometry != ALEX_GEOMETRY {
        //todo convert geometry?
        return invalid_geometry().to_term(env);
    }

    let xuid = last_data["extraData"]["XUID"].as_str();

    let png = lodepng::encode32(raw_skin_data.as_slice(), skin_width, skin_height).unwrap();

    let mut hasher = Sha256::new();
    hasher.update(raw_skin_data.as_slice());

    let hash = hasher.finalize();

    make_tuple(env, &[xuid.encode(env), is_steve.to_term(env), as_binary(env, &png), as_binary(env, hash.as_slice())])
}

fn as_binary<'a>(env: Env<'a>, data: &[u8]) -> Term<'a> {
    let mut erl_bin: OwnedBinary = OwnedBinary::new(data.len()).unwrap();
    erl_bin.as_mut_slice().copy_from_slice(data);
    Binary::from_owned(erl_bin, env).to_term(env)
}

pub fn create_key(pub_key: &primitive::str) -> Algorithm {
    Algorithm::new_ecdsa_pem_verifier(AlgorithmID::ES384, create_key_from(pub_key).as_bytes()).unwrap()
}

pub fn create_key_from<'a>(pub_key: &primitive::str) -> String {
    vec!["-----BEGIN PUBLIC KEY-----", pub_key, "-----END PUBLIC KEY-----"].concat()
}

rustler::init!("Elixir.GlobalLinking.SkinNifUtils", [validate_and_get_png]);
