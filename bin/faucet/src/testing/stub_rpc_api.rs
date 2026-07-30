use std::sync::OnceLock;

use anyhow::Context;
use miden_client::block::{BlockHeader, ValidatorKeys};
use miden_client::crypto::ecdsa_k256_keccak::SigningKey;
use miden_client::crypto::eddsa_25519_sha512::KeyExchangeKey;
use miden_client::rpc::encryption::attestation_commitment;
use miden_client::utils::Serializable;
use miden_testing::MockChain;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status};
use tonic_web::GrpcWebLayer;
use url::Url;

use super::proto;
use super::proto::rpc::api_server;
use super::proto::to_proto_block_header;

/// Wire identifier of `IES_SCHEME_X25519_XCHACHA20_POLY1305`, the scheme the client seals for.
const IES_SCHEME_X25519_XCHACHA20_POLY1305: i32 = 1;

/// Chain state served by the stub.
///
/// The header is stable across requests and commits to a validator signing key the stub holds,
/// so the transaction encryption key it serves carries an attestation the client can verify
/// against the header it previously stored.
struct StubChain {
    genesis: BlockHeader,
    validator_signer: SigningKey,
    encryption_key: KeyExchangeKey,
}

fn stub_chain() -> &'static StubChain {
    static STATE: OnceLock<StubChain> = OnceLock::new();
    STATE.get_or_init(|| {
        let validator_signer = SigningKey::new();
        let encryption_key = KeyExchangeKey::new();
        let validator_keys = ValidatorKeys::new(vec![validator_signer.public_key()])
            .expect("a single-key validator set is valid");

        // Only the validator set matters to the stub's consumers; every other field is taken
        // from a mock header so the roots stay well-formed.
        let template = MockChain::new().latest_block_header();
        let genesis = BlockHeader::new(
            template.version(),
            template.prev_block_commitment(),
            template.block_num(),
            template.chain_commitment(),
            template.account_root(),
            template.nullifier_root(),
            template.note_root(),
            template.tx_commitment(),
            template.tx_kernel_commitment(),
            validator_keys,
            template.fee_parameters().clone(),
            template.timestamp(),
        );

        StubChain {
            genesis,
            validator_signer,
            encryption_key,
        }
    })
}

pub struct StubRpcApi;

#[tonic::async_trait]
impl api_server::Api for StubRpcApi {
    async fn get_block_header_by_number(
        &self,
        _request: Request<proto::rpc::BlockHeaderByNumberRequest>,
    ) -> Result<Response<proto::rpc::BlockHeaderByNumberResponse>, Status> {
        Ok(Response::new(proto::rpc::BlockHeaderByNumberResponse {
            block_header: Some(to_proto_block_header(&stub_chain().genesis)),
            mmr_path: None,
            chain_length: None,
        }))
    }

    async fn get_transaction_encryption_key(
        &self,
        _request: Request<()>,
    ) -> Result<Response<proto::transaction::TransactionEncryptionKey>, Status> {
        let chain = stub_chain();
        let key_id = b"stub-key-id".to_vec();
        let public_key = chain.encryption_key.public_key().to_bytes();

        // The commitment layout is mirrored from the validator; signing it with the key
        // committed in the stub's header makes the attestation verify on the client.
        let commitment = attestation_commitment(
            IES_SCHEME_X25519_XCHACHA20_POLY1305 as u32,
            &key_id,
            chain.genesis.commitment(),
            &public_key,
            None,
        );
        let signature = chain.validator_signer.sign(commitment);

        Ok(Response::new(proto::transaction::TransactionEncryptionKey {
            scheme: IES_SCHEME_X25519_XCHACHA20_POLY1305,
            key_id,
            public_key,
            attestations: vec![proto::transaction::ValidatorKeyAttestation {
                validator_public_key: chain.validator_signer.public_key().to_bytes(),
                signature: signature.to_bytes(),
            }],
            next_key: None,
        }))
    }

    async fn sync_notes(
        &self,
        _request: Request<proto::rpc::SyncNotesRequest>,
    ) -> Result<Response<proto::rpc::SyncNotesResponse>, Status> {
        Ok(Response::new(proto::rpc::SyncNotesResponse {
            pagination_info: Some(proto::rpc::PaginationInfo { chain_tip: 0, block_num: 0 }),
            blocks: vec![],
        }))
    }

    async fn get_notes_by_id(
        &self,
        _request: Request<proto::note::NoteIdList>,
    ) -> Result<Response<proto::note::CommittedNoteList>, Status> {
        unimplemented!()
    }

    async fn submit_proven_tx(
        &self,
        _request: Request<proto::transaction::ProvenTransaction>,
    ) -> Result<Response<proto::blockchain::BlockNumber>, Status> {
        Ok(Response::new(proto::blockchain::BlockNumber { block_num: 0 }))
    }

    async fn submit_proven_tx_batch(
        &self,
        _request: Request<proto::transaction::TransactionBatch>,
    ) -> Result<Response<proto::blockchain::BlockNumber>, Status> {
        unimplemented!()
    }

    type BlockSubscriptionStream = std::pin::Pin<
        Box<
            dyn tokio_stream::Stream<Item = Result<proto::rpc::BlockSubscriptionResponse, Status>>
                + Send,
        >,
    >;

    async fn block_subscription(
        &self,
        _request: Request<proto::rpc::BlockSubscriptionRequest>,
    ) -> Result<Response<Self::BlockSubscriptionStream>, Status> {
        unimplemented!()
    }

    type ProofSubscriptionStream = std::pin::Pin<
        Box<
            dyn tokio_stream::Stream<Item = Result<proto::rpc::ProofSubscriptionResponse, Status>>
                + Send,
        >,
    >;

    async fn proof_subscription(
        &self,
        _request: Request<proto::rpc::ProofSubscriptionRequest>,
    ) -> Result<Response<Self::ProofSubscriptionStream>, Status> {
        unimplemented!()
    }

    async fn get_account(
        &self,
        _request: Request<proto::rpc::AccountRequest>,
    ) -> Result<Response<proto::rpc::AccountResponse>, Status> {
        Err(Status::not_found("account not found"))
    }

    async fn get_block_by_number(
        &self,
        _request: Request<proto::blockchain::BlockRequest>,
    ) -> Result<Response<proto::blockchain::MaybeBlock>, Status> {
        unimplemented!()
    }

    async fn status(
        &self,
        _request: Request<()>,
    ) -> Result<Response<proto::rpc::RpcStatus>, Status> {
        unimplemented!()
    }

    async fn sync_account_vault(
        &self,
        _request: Request<proto::rpc::SyncAccountVaultRequest>,
    ) -> Result<Response<proto::rpc::SyncAccountVaultResponse>, Status> {
        unimplemented!()
    }

    async fn sync_account_storage_maps(
        &self,
        _request: Request<proto::rpc::SyncAccountStorageMapsRequest>,
    ) -> Result<Response<proto::rpc::SyncAccountStorageMapsResponse>, Status> {
        unimplemented!()
    }

    async fn get_note_script_by_root(
        &self,
        _request: Request<proto::note::NoteScriptRoot>,
    ) -> Result<Response<proto::rpc::MaybeNoteScript>, Status> {
        unimplemented!()
    }

    async fn sync_nullifiers(
        &self,
        _request: Request<proto::rpc::SyncNullifiersRequest>,
    ) -> Result<Response<proto::rpc::SyncNullifiersResponse>, Status> {
        Ok(Response::new(proto::rpc::SyncNullifiersResponse {
            nullifiers: vec![],
            pagination_info: Some(proto::rpc::PaginationInfo { chain_tip: 0, block_num: 0 }),
        }))
    }

    async fn sync_transactions(
        &self,
        _request: Request<proto::rpc::SyncTransactionsRequest>,
    ) -> Result<Response<proto::rpc::SyncTransactionsResponse>, Status> {
        unimplemented!()
    }

    async fn get_limits(
        &self,
        _request: Request<()>,
    ) -> Result<Response<proto::rpc::RpcLimits>, Status> {
        use std::collections::HashMap;

        let make_endpoint = |params: Vec<(&str, u32)>| proto::rpc::EndpointLimits {
            parameters: params.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        };

        let endpoints = HashMap::from([
            ("GetNotesById".to_string(), make_endpoint(vec![("note_id", 100)])),
            ("CheckNullifiers".to_string(), make_endpoint(vec![("nullifier", 1000)])),
            ("SyncNullifiers".to_string(), make_endpoint(vec![("nullifier", 1000)])),
            ("SyncTransactions".to_string(), make_endpoint(vec![("account_id", 1000)])),
            ("SyncNotes".to_string(), make_endpoint(vec![("note_tag", 1000)])),
        ]);

        Ok(Response::new(proto::rpc::RpcLimits { endpoints }))
    }

    async fn sync_chain_mmr(
        &self,
        _request: Request<proto::rpc::SyncChainMmrRequest>,
    ) -> Result<Response<proto::rpc::SyncChainMmrResponse>, Status> {
        Ok(Response::new(proto::rpc::SyncChainMmrResponse {
            block_range: Some(proto::rpc::BlockRange { block_from: 0, block_to: 0 }),
            mmr_delta: Some(proto::primitives::MmrDelta { forest: 0, data: vec![] }),
            block_header: Some(to_proto_block_header(&stub_chain().genesis)),
            block_signatures: vec![],
        }))
    }

    async fn get_network_note_status(
        &self,
        _request: Request<proto::note::NoteId>,
    ) -> Result<Response<proto::rpc::GetNetworkNoteStatusResponse>, Status> {
        unimplemented!()
    }
}

pub async fn serve_stub(endpoint: &Url) -> anyhow::Result<()> {
    let addr = endpoint
        .socket_addrs(|| None)
        .context("failed to convert endpoint to socket address")?
        .into_iter()
        .next()
        .unwrap();

    let listener = TcpListener::bind(addr).await?;
    let api_service = api_server::ApiServer::new(StubRpcApi);

    tonic::transport::Server::builder()
        .accept_http1(true)
        .layer(GrpcWebLayer::new())
        .add_service(api_service)
        .serve_with_incoming(TcpListenerStream::new(listener))
        .await
        .context("failed to serve stub RPC API")
}
