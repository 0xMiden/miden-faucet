use anyhow::Context;
use miden_client::block::BlockHeader;
use miden_client::crypto::{Forest, MmrDelta};
use miden_client::utils::{Deserializable, Serializable};
use miden_node_proto::generated::rpc::api_server;
use miden_node_proto::generated::{self as proto};
use miden_testing::MockChain;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status};
use tonic_web::GrpcWebLayer;
use url::Url;

pub struct StubRpcApi;

#[tonic::async_trait]
impl api_server::Api for StubRpcApi {
    async fn check_nullifiers(
        &self,
        _request: Request<proto::rpc::NullifierList>,
    ) -> Result<Response<proto::rpc::CheckNullifiersResponse>, Status> {
        unimplemented!();
    }

    async fn get_block_header_by_number(
        &self,
        _request: Request<proto::rpc::BlockHeaderByNumberRequest>,
    ) -> Result<Response<proto::rpc::BlockHeaderByNumberResponse>, Status> {
        let mock_chain = MockChain::new();
        let protocol_header = mock_chain.latest_block_header();

        let bytes = protocol_header.to_bytes();
        let client_header = BlockHeader::read_from_bytes(&bytes)
            .map_err(|e| Status::internal(format!("Failed to deserialize block header: {e}")))?;

        Ok(Response::new(proto::rpc::BlockHeaderByNumberResponse {
            block_header: Some((&client_header).into()),
            mmr_path: None,
            chain_length: None,
        }))
    }

    async fn sync_state(
        &self,
        _request: Request<proto::rpc::SyncStateRequest>,
    ) -> Result<Response<proto::rpc::SyncStateResponse>, Status> {
        let mock_chain = MockChain::new();
        let protocol_header = mock_chain.latest_block_header();
        let mmr_peaks = mock_chain.blockchain().peaks_at(protocol_header.block_num()).unwrap();
        let mmr_delta: MmrDelta = mock_chain
            .blockchain()
            .as_mmr()
            .get_delta(Forest::empty(), mmr_peaks.forest())
            .unwrap();

        let bytes = protocol_header.to_bytes();
        let client_header = BlockHeader::read_from_bytes(&bytes)
            .map_err(|e| Status::internal(format!("Failed to deserialize block header: {e}")))?;

        Ok(Response::new(proto::rpc::SyncStateResponse {
            chain_tip: client_header.block_num().as_u32(),
            block_header: Some((&client_header).into()),
            mmr_delta: Some(mmr_delta.into()),
            accounts: vec![],
            transactions: vec![],
            notes: vec![],
        }))
    }

    async fn sync_notes(
        &self,
        _request: Request<proto::rpc::SyncNotesRequest>,
    ) -> Result<Response<proto::rpc::SyncNotesResponse>, Status> {
        unimplemented!();
    }

    async fn get_notes_by_id(
        &self,
        _request: Request<proto::note::NoteIdList>,
    ) -> Result<Response<proto::note::CommittedNoteList>, Status> {
        unimplemented!();
    }

    async fn submit_proven_transaction(
        &self,
        _request: Request<proto::transaction::ProvenTransaction>,
    ) -> Result<Response<proto::blockchain::BlockNumber>, Status> {
        Ok(Response::new(proto::blockchain::BlockNumber { block_num: 0 }))
    }

    async fn submit_proven_batch(
        &self,
        _request: Request<proto::transaction::ProvenTransactionBatch>,
    ) -> Result<Response<proto::blockchain::BlockNumber>, Status> {
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
        _request: Request<proto::blockchain::BlockNumber>,
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
        _request: Request<proto::note::NoteRoot>,
    ) -> Result<Response<proto::rpc::MaybeNoteScript>, Status> {
        unimplemented!()
    }

    async fn sync_nullifiers(
        &self,
        _request: Request<proto::rpc::SyncNullifiersRequest>,
    ) -> Result<Response<proto::rpc::SyncNullifiersResponse>, Status> {
        Ok(Response::new(proto::rpc::SyncNullifiersResponse {
            nullifiers: vec![],
            pagination_info: None,
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
