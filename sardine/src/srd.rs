use std;
use std::io::Write;

use rand::{OsRng, Rng};

use num_bigint::BigUint;

use digest::Digest;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use Result;
use srd_errors::SrdError;
use message_types::*;
use srd_blob::{Blob, SrdBlob};
use dh_params::SRD_DH_PARAMS;

pub struct Srd {
    blob: Option<SrdBlob>,

    is_server: bool,
    key_size: u16,
    seq_num: u8,
    state: u8,

    messages: Vec<Box<SrdPacket>>,

    cert_data: Option<Vec<u8>>,

    client_nonce: [u8; 32],
    server_nonce: [u8; 32],
    delegation_key: [u8; 32],
    integrity_key: [u8; 32],
    iv: [u8; 16],

    generator: BigUint,

    prime: BigUint,
    private_key: BigUint,
    secret_key: Vec<u8>,

    rng: OsRng,
}

impl Srd {
    pub fn new(is_server: bool) -> Result<Srd> {
        Ok(Srd {
            blob: None,

            is_server,
            key_size: 256,
            seq_num: 0,
            state: 0,

            messages: Vec::new(),

            cert_data: None,

            client_nonce: [0; 32],
            server_nonce: [0; 32],
            delegation_key: [0; 32],
            integrity_key: [0; 32],
            iv: [0; 16],

            generator: BigUint::from_bytes_be(&[0]),

            prime: BigUint::from_bytes_be(&[0]),
            private_key: BigUint::from_bytes_be(&[0]),
            secret_key: Vec::new(),

            rng: OsRng::new()?,
        })
    }

    pub fn get_blob<T: Blob>(&self) -> Result<Option<T>> {
        if self.blob.is_some() {
            let blob = self.blob.as_ref().unwrap();
            if blob.blob_type == T::blob_type() {
                let mut cursor = std::io::Cursor::new(blob.data.as_slice());
                return Ok(Some(T::read_from(&mut cursor)?));
            }
        }

        Ok(None)
    }

    pub fn blob(&self) -> &Option<SrdBlob> {
        &self.blob
    }

    pub fn set_blob<T: Blob>(&mut self, blob: T) -> Result<()> {
        let mut data = Vec::new();
        blob.write_to(&mut data)?;
        self.blob = Some(SrdBlob::new(T::blob_type(), &data));
        Ok(())
    }

    pub fn set_cert_data(&mut self, buffer: Vec<u8>) -> Result<()> {
        self.cert_data = Some(buffer);
        Ok(())
    }

    pub fn set_key_size(&mut self, key_size: u16) -> Result<()> {
        match key_size {
            256 | 512 | 1024 => {
                self.key_size = key_size;
                Ok(())
            }
            _ => Err(SrdError::InvalidKeySize),
        }
    }

    fn write_msg<T: SrdPacket>(&mut self, msg: &T, buffer: &mut Vec<u8>) -> Result<()> {
        if msg.signature() != SRD_SIGNATURE {
            return Err(SrdError::InvalidSignature);
        }

        if msg.seq_num() != self.seq_num {
            return Err(SrdError::BadSequence);
        }

        msg.write_to(buffer)?;
        self.seq_num += 1;
        Ok(())
    }

    fn read_msg<T: SrdPacket>(&mut self, buffer: &[u8]) -> Result<T>
    where
        T: SrdPacket,
    {
        let mut reader = std::io::Cursor::new(buffer);
        let packet = T::read_from(&mut reader)?;

        if packet.signature() != SRD_SIGNATURE {
            return Err(SrdError::InvalidSignature);
        }

        if packet.seq_num() != self.seq_num {
            return Err(SrdError::BadSequence);
        }

        self.seq_num += 1;

        Ok(packet)
    }

    pub fn authenticate(&mut self, input_data: &[u8], output_data: &mut Vec<u8>) -> Result<bool> {
        if self.is_server {
            match self.state {
                0 => self.server_authenticate_0(input_data, output_data)?,
                1 => self.server_authenticate_1(input_data, output_data)?,
                2 => {
                    self.server_authenticate_2(input_data)?;
                    self.state += 1;
                    return Ok(true);
                }
                _ => return Err(SrdError::BadSequence),
            }
        } else {
            match self.state {
                0 => self.client_authenticate_0(output_data)?,
                1 => self.client_authenticate_1(input_data, output_data)?,
                2 => {
                    self.client_authenticate_2(input_data, output_data)?;
                    self.state += 1;
                    return Ok(true);
                }
                _ => return Err(SrdError::BadSequence),
            }
        }
        self.state += 1;
        Ok(false)
    }

    // Client initiate
    fn client_authenticate_0(&mut self, mut output_data: &mut Vec<u8>) -> Result<()> {
        // Negotiate
        let out_packet = SrdInitiate::new(self.seq_num, self.key_size);
        self.write_msg(&out_packet, &mut output_data)?;

        self.messages.push(Box::new(out_packet));
        Ok(())
    }

    // Server initiate -> offer
    fn server_authenticate_0(
        &mut self,
        input_data: &[u8],
        mut output_data: &mut Vec<u8>,
    ) -> Result<()> {
        // Negotiate
        let in_packet = self.read_msg::<SrdInitiate>(input_data)?;
        self.set_key_size(in_packet.key_size())?;
        self.find_dh_parameters()?;

        let key_size = in_packet.key_size();

        self.messages.push(Box::new(in_packet));

        // Challenge
        let mut private_key_bytes = vec![0u8; self.key_size as usize];
        self.rng.fill_bytes(&mut private_key_bytes);
        self.private_key = BigUint::from_bytes_be(&private_key_bytes);

        let public_key = self.generator.modpow(&self.private_key, &self.prime);

        self.rng.fill_bytes(&mut self.server_nonce);

        let out_packet = SrdOffer::new(
            self.seq_num,
            key_size,
            self.generator.to_bytes_be(),
            self.prime.to_bytes_be(),
            public_key.to_bytes_be(),
            self.server_nonce,
        );

        self.write_msg(&out_packet, &mut output_data)?;

        self.messages.push(Box::new(out_packet));

        Ok(())
    }

    // Client offer -> accept
    fn client_authenticate_1(
        &mut self,
        input_data: &[u8],
        mut output_data: &mut Vec<u8>,
    ) -> Result<()> {
        //Challenge
        let in_packet = self.read_msg::<SrdOffer>(input_data)?;

        self.generator = BigUint::from_bytes_be(&in_packet.generator);
        self.prime = BigUint::from_bytes_be(&in_packet.prime);

        let mut private_key_bytes = vec![0u8; self.key_size as usize];
        self.rng.fill_bytes(&mut private_key_bytes);
        self.private_key = BigUint::from_bytes_be(&private_key_bytes);

        let public_key = self.generator.modpow(&self.private_key, &self.prime);

        self.rng.fill_bytes(&mut self.client_nonce);

        self.server_nonce = in_packet.nonce;
        self.secret_key = BigUint::from_bytes_be(&in_packet.public_key)
            .modpow(&self.private_key, &self.prime)
            .to_bytes_be();

        self.derive_keys();

        let key_size = in_packet.key_size();

        self.messages.push(Box::new(in_packet));

        // Generate cbt
        let cbt;

        match self.cert_data {
            None => cbt = None,
            Some(ref cert) => {
                let mut hmac = Hmac::<Sha256>::new_varkey(&self.integrity_key)?;

                hmac.input(&self.client_nonce);
                hmac.input(&cert);

                let mut cbt_data: [u8; 32] = [0u8; 32];
                hmac.result().code().to_vec().write_all(&mut cbt_data)?;
                cbt = Some(cbt_data);
            }
        }

        let out_packet = SrdAccept::new(
            self.seq_num,
            key_size,
            public_key.to_bytes_be(),
            self.client_nonce,
            cbt,
            &self.messages,
            &self.integrity_key,
        )?;

        self.write_msg(&out_packet, &mut output_data)?;

        self.messages.push(Box::new(out_packet));

        Ok(())
    }

    // Server accept -> confirm
    fn server_authenticate_1(
        &mut self,
        input_data: &[u8],
        mut output_data: &mut Vec<u8>,
    ) -> Result<()> {
        // Response
        let in_packet = self.read_msg::<SrdAccept>(input_data)?;
        self.client_nonce = in_packet.nonce;

        self.secret_key = BigUint::from_bytes_be(&in_packet.public_key)
            .modpow(&self.private_key, &self.prime)
            .to_bytes_be();

        self.derive_keys();

        in_packet.verify_mac(&self.messages, &self.integrity_key)?;

        // Verify client cbt
        match self.cert_data {
            None => {
                if in_packet.has_cbt() {
                    return Err(SrdError::InvalidCert);
                }
            }
            Some(ref c) => {
                if !in_packet.has_cbt() {
                    return Err(SrdError::InvalidCert);
                }
                let mut hmac = Hmac::<Sha256>::new_varkey(&self.integrity_key)?;

                hmac.input(&self.client_nonce);
                hmac.input(&c);

                let mut cbt_data: [u8; 32] = [0u8; 32];
                hmac.result().code().to_vec().write_all(&mut cbt_data)?;
                if cbt_data != in_packet.cbt {
                    return Err(SrdError::InvalidCbt);
                }
            }
        }

        self.messages.push(Box::new(in_packet));

        // Confirm
        // Generate server cbt
        let cbt;
        match self.cert_data {
            None => cbt = None,
            Some(ref cert) => {
                let mut hmac = Hmac::<Sha256>::new_varkey(&self.integrity_key)?;

                hmac.input(&self.server_nonce);
                hmac.input(&cert);

                let mut cbt_data: [u8; 32] = [0u8; 32];
                hmac.result().code().to_vec().write_all(&mut cbt_data)?;
                cbt = Some(cbt_data);
            }
        }

        let out_packet = SrdConfirm::new(self.seq_num, cbt, &self.messages, &self.integrity_key)?;

        self.write_msg(&out_packet, &mut output_data)?;

        self.messages.push(Box::new(out_packet));

        Ok(())
    }

    // Client confirm -> delegate
    fn client_authenticate_2(
        &mut self,
        input_data: &[u8],
        mut output_data: &mut Vec<u8>,
    ) -> Result<()> {
        // Confirm
        let in_packet = self.read_msg::<SrdConfirm>(input_data)?;

        in_packet.verify_mac(&self.messages, &self.integrity_key)?;

        // Verify Server cbt
        match self.cert_data {
            None => {
                if in_packet.has_cbt() {
                    return Err(SrdError::InvalidCert);
                }
            }
            Some(ref c) => {
                if !in_packet.has_cbt() {
                    return Err(SrdError::InvalidCert);
                }
                let mut hmac = Hmac::<Sha256>::new_varkey(&self.integrity_key)?;

                hmac.input(&self.server_nonce);
                hmac.input(&c);

                let mut cbt_data: [u8; 32] = [0u8; 32];
                hmac.result().code().to_vec().write_all(&mut cbt_data)?;
                if cbt_data != in_packet.cbt {
                    return Err(SrdError::InvalidCbt);
                }
            }
        }

        self.messages.push(Box::new(in_packet));

        let out_packet: SrdDelegate;
        // Delegate
        match self.blob {
            None => {
                return Err(SrdError::MissingBlob);
            }
            Some(ref b) => {
                out_packet = SrdDelegate::new(
                    self.seq_num,
                    b,
                    &self.messages,
                    &self.integrity_key,
                    &self.delegation_key,
                    &self.iv,
                )?;
            }
        }

        self.write_msg(&out_packet, &mut output_data)?;
        self.messages.push(Box::new(out_packet));
        Ok(())
    }

    // Server delegate -> result
    fn server_authenticate_2(&mut self, input_data: &[u8]) -> Result<()> {
        // Receive delegate and verify credentials...
        let in_packet = self.read_msg::<SrdDelegate>(input_data)?;
        in_packet.verify_mac(&self.messages, &self.integrity_key)?;

        self.blob = Some(in_packet.get_data(&self.delegation_key, &self.iv[0..16])?);

        self.messages.push(Box::new(in_packet));

        Ok(())
    }

    fn find_dh_parameters(&mut self) -> Result<()> {
        match self.key_size {
            256 => {
                self.generator = BigUint::from_bytes_be(SRD_DH_PARAMS[0].g_data);
                self.prime = BigUint::from_bytes_be(SRD_DH_PARAMS[0].p_data);
                Ok(())
            }
            512 => {
                self.generator = BigUint::from_bytes_be(SRD_DH_PARAMS[1].g_data);
                self.prime = BigUint::from_bytes_be(SRD_DH_PARAMS[1].p_data);
                Ok(())
            }
            1024 => {
                self.generator = BigUint::from_bytes_be(SRD_DH_PARAMS[2].g_data);
                self.prime = BigUint::from_bytes_be(SRD_DH_PARAMS[2].p_data);
                Ok(())
            }
            _ => Err(SrdError::InvalidKeySize),
        }
    }

    fn derive_keys(&mut self) {
        let mut hash = Sha256::new();
        hash.input(&self.client_nonce);
        hash.input(&self.secret_key);
        hash.input(&self.server_nonce);

        self.delegation_key
            .clone_from_slice(&hash.result().to_vec());

        hash = Sha256::new();
        hash.input(&self.server_nonce);
        hash.input(&self.secret_key);
        hash.input(&self.client_nonce);

        self.integrity_key.clone_from_slice(&hash.result().to_vec());

        hash = Sha256::new();
        hash.input(&self.client_nonce);
        hash.input(&self.server_nonce);

        self.iv.clone_from_slice(&hash.result().to_vec()[0..16]);
    }
}