use anyhow::Context;
use miden_client::crypto::{Forest, MmrDelta};
use miden_node_proto::generated::rpc::api_server;
use miden_node_proto::generated::{self as proto};
use miden_testing::MockChain;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status};
use tonic_web::GrpcWebLayer;
use url::Url;

#[derive(Clone)]
pub struct StubRpcApi;

#[tonic::async_trait]
impl api_server::Api for StubRpcApi {
    async fn check_nullifiers(
        &self,
        _request: Request<proto::rpc_store::NullifierList>,
    ) -> Result<Response<proto::rpc_store::CheckNullifiersResponse>, Status> {
        unimplemented!();
    }

    async fn get_block_header_by_number(
        &self,
        _request: Request<proto::shared::BlockHeaderByNumberRequest>,
    ) -> Result<Response<proto::shared::BlockHeaderByNumberResponse>, Status> {
        let mock_chain = MockChain::new();

        let block_header =
            proto::blockchain::BlockHeader::from(mock_chain.latest_block_header()).into();

        Ok(Response::new(proto::shared::BlockHeaderByNumberResponse {
            block_header,
            mmr_path: None,
            chain_length: None,
        }))
    }

    async fn sync_state(
        &self,
        _request: Request<proto::rpc_store::SyncStateRequest>,
    ) -> Result<Response<proto::rpc_store::SyncStateResponse>, Status> {
        let mock_chain = MockChain::new();
        let block_header = proto::blockchain::BlockHeader::from(mock_chain.latest_block_header());
        let mmr_peaks = mock_chain.blockchain().peaks_at(block_header.block_num.into()).unwrap();
        let mmr_delta: MmrDelta = mock_chain
            .blockchain()
            .as_mmr()
            .get_delta(Forest::empty(), mmr_peaks.forest())
            .unwrap();

        Ok(Response::new(proto::rpc_store::SyncStateResponse {
            chain_tip: 0,
            block_header: Some(block_header),
            mmr_delta: Some(mmr_delta.into()),
            accounts: vec![],
            transactions: vec![],
            notes: vec![],
        }))
    }

    async fn sync_notes(
        &self,
        _request: Request<proto::rpc_store::SyncNotesRequest>,
    ) -> Result<Response<proto::rpc_store::SyncNotesResponse>, Status> {
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
    ) -> Result<Response<proto::block_producer::SubmitProvenTransactionResponse>, Status> {
        Ok(Response::new(proto::block_producer::SubmitProvenTransactionResponse {
            block_height: 0,
        }))
    }

    async fn submit_proven_batch(
        &self,
        _request: Request<proto::transaction::ProvenTransactionBatch>,
    ) -> Result<Response<proto::block_producer::SubmitProvenBatchResponse>, Status> {
        unimplemented!()
    }

    async fn get_account_details(
        &self,
        _request: Request<proto::account::AccountId>,
    ) -> Result<Response<proto::account::AccountDetails>, Status> {
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
        _request: Request<proto::rpc_store::SyncAccountVaultRequest>,
    ) -> Result<Response<proto::rpc_store::SyncAccountVaultResponse>, Status> {
        unimplemented!()
    }

    async fn sync_storage_maps(
        &self,
        _request: Request<proto::rpc_store::SyncStorageMapsRequest>,
    ) -> Result<Response<proto::rpc_store::SyncStorageMapsResponse>, Status> {
        unimplemented!()
    }

    async fn get_account_proof(
        &self,
        _request: Request<proto::rpc_store::AccountProofRequest>,
    ) -> Result<Response<proto::rpc_store::AccountProofResponse>, Status> {
        unimplemented!()
    }

    async fn get_note_script_by_root(
        &self,
        _request: Request<proto::note::NoteRoot>,
    ) -> Result<Response<proto::shared::MaybeNoteScript>, Status> {
        unimplemented!()
    }

    async fn sync_nullifiers(
        &self,
        _request: Request<proto::rpc_store::SyncNullifiersRequest>,
    ) -> Result<Response<proto::rpc_store::SyncNullifiersResponse>, Status> {
        Ok(Response::new(proto::rpc_store::SyncNullifiersResponse {
            nullifiers: vec![],
            pagination_info: None,
        }))
    }

    async fn sync_transactions(
        &self,
        _request: Request<proto::rpc_store::SyncTransactionsRequest>,
    ) -> Result<Response<proto::rpc_store::SyncTransactionsResponse>, Status> {
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
