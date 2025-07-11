use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        CompactSize { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self.value {
            v if v <= 0xFC => vec![v as u8],
            v if v <= 0xFFFF => {
                let mut bytes = vec![0xFD];
                bytes.extend_from_slice(&(v as u16).to_le_bytes());
                bytes
            }
            v if v <= 0xFFFFFFFF => {
                let mut bytes = vec![0xFE];
                bytes.extend_from_slice(&(v as u32).to_le_bytes());
                bytes
            }
            v => {
                let mut bytes = vec![0xFF];
                bytes.extend_from_slice(&v.to_le_bytes());
                bytes
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }

        match bytes[0] {
            0xFD => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u16::from_le_bytes([bytes[1], bytes[2]]);
                Ok((CompactSize::new(value as u64), 3))
            }
            0xFE => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                Ok((CompactSize::new(value as u64), 5))
            }
            0xFF => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u64::from_le_bytes([
                    bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8],
                ]);
                Ok((CompactSize::new(value), 9))
            }
            x => Ok((CompactSize::new(x as u64), 1)),
        }
    }
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&hex::encode(self.0))
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(s)
            .map_err(serde::de::Error::custom)
            .and_then(|bytes| {
                if bytes.len() == 32 {
                    let mut txid = [0u8; 32];
                    txid.copy_from_slice(&bytes);
                    Ok(Txid(txid))
                } else {
                    Err(serde::de::Error::custom(
                        "Txid must be 32 bytes (64 hex characters)",
                    ))
                }
            })
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        OutPoint {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.txid.0.to_vec();
        bytes.extend_from_slice(&self.vout.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes[0..32]);
        let vout = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);
        Ok((OutPoint::new(txid, vout), 36))
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Default)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Script { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = CompactSize::new(self.bytes.len() as u64).to_bytes();
        bytes.extend_from_slice(&self.bytes);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (size, size_bytes) = CompactSize::from_bytes(bytes)?;
        let script_len = size.value as usize;
        if bytes.len() < size_bytes + script_len {
            return Err(BitcoinError::InsufficientBytes);
        }
        let script_bytes = bytes[size_bytes..size_bytes + script_len].to_vec();
        Ok((Script::new(script_bytes), size_bytes + script_len))
    }
}
impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        TransactionInput {
            previous_output,
            script_sig,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.previous_output.to_bytes();
        bytes.extend(self.script_sig.to_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (previous_output, outpoint_bytes) = OutPoint::from_bytes(bytes)?;
        let (script_sig, script_bytes) = Script::from_bytes(&bytes[outpoint_bytes..])?;

        if bytes.len() < outpoint_bytes + script_bytes + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let sequence = u32::from_le_bytes([
            bytes[outpoint_bytes + script_bytes],
            bytes[outpoint_bytes + script_bytes + 1],
            bytes[outpoint_bytes + script_bytes + 2],
            bytes[outpoint_bytes + script_bytes + 3],
        ]);
        Ok((
            Self::new(previous_output, script_sig, sequence),
            outpoint_bytes + script_bytes + 4,
        ))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        BitcoinTransaction {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.version.to_le_bytes().to_vec();
        bytes.extend(CompactSize::new(self.inputs.len() as u64).to_bytes());
        for input in &self.inputs {
            bytes.extend(input.to_bytes());
        }
        bytes.extend_from_slice(&self.lock_time.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 8 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let (input_count, mut offset) = CompactSize::from_bytes(&bytes[4..])?;
        offset += 4;

        let mut inputs = Vec::new();
        for _ in 0..input_count.value {
            let (input, input_bytes) = TransactionInput::from_bytes(&bytes[offset..])?;
            inputs.push(input);
            offset += input_bytes;
        }

        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let lock_time = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        Ok((
            BitcoinTransaction::new(version, inputs, lock_time),
            offset + 4,
        ))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bitcoin Transaction:\n")?;
        write!(f, "  Version: {}\n", self.version)?;
        write!(f, "  Inputs:\n")?;
        for (i, input) in self.inputs.iter().enumerate() {
            write!(f, "    Input {}:\n", i + 1)?;
            write!(
                f,
                "      Previous Output Vout: {}\n",
                input.previous_output.vout
            )?;
            write!(
                f,
                "      ScriptSig: length={}, bytes={}\n",
                input.script_sig.len(),
                hex::encode(&*input.script_sig)
            )?;
            write!(f, "      Sequence: {}\n", input.sequence)?;
        }
        write!(f, "  Lock Time: {}\n", self.lock_time)
    }
}
