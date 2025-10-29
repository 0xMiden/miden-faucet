import { MidenWalletAdapter } from "@demox-labs/miden-wallet-adapter-miden";
import { PrivateDataPermission, WalletAdapterNetwork } from "@demox-labs/miden-wallet-adapter-base";
import { Endpoint, NoteId, RpcClient } from "@demox-labs/miden-sdk";
import { Utils } from './utils.js';
import { UIController } from './ui.js';
import { getConfig, getMetadata, getPowChallenge, getTokens, get_note } from "./api.js";

export class MidenFaucetApp {
    constructor() {
        this.ui = new UIController();
        this.tokenAmountOptions = [100, 500, 1000];
        this.metadataInitialized = false;
        this.apiUrl = null;
        this.rpcClient = null;
        this.baseAmount = null;
        this.powLoadDifficulty = null;

        // Check if Web Crypto API is available
        if (!window.crypto || !window.crypto.subtle) {
            console.error("Web Crypto API not available");
            this.ui.showError('Web Crypto API not available. Please use a modern browser.');
        }

        this.walletAdapter = new MidenWalletAdapter({ appName: 'Miden Faucet' });

        this.init();
    }

    async init() {
        try {
            let config = await getConfig();
            this.apiUrl = config.api_url;
            this.rpcClient = new RpcClient(new Endpoint(config.node_url));
            this.setupEventListeners();
            this.startMetadataPolling();
        } catch (error) {
            console.error('Failed to initialize app:', error);
            this.ui.showError('Failed to initialize application. Please refresh the page.');
        }
    }

    setupEventListeners() {
        const onSendTokens = (isPrivateNote) => this.handleSendTokens(isPrivateNote);
        const onWalletConnect = () => this.handleWalletConnect();
        const onTokenSelect = (requestedAmount) => this.updateTokenHint(requestedAmount);
        this.ui.setupEventListeners(onSendTokens, onWalletConnect, onTokenSelect);
    }


    async handleWalletConnect() {
        try {
            await this.walletAdapter.connect(PrivateDataPermission.UponRequest, WalletAdapterNetwork.Testnet);

            if (this.walletAdapter.accountId) {
                this.ui.setRecipientAddress(this.walletAdapter.accountId);
            }
        } catch (error) {
            console.error("WalletConnectionError:", error);
            this.ui.showError("Failed to connect wallet.");
        }
    }

    async handleSendTokens(isPrivateNote) {
        try {
            const { recipient, amount, amountAsTokens } = this.ui.getFormData();

            if (!recipient) {
                this.ui.showError('Recipient address is required.');
                return;
            }
            if (!amount || amount === '0') {
                this.ui.showError('Amount is required.');
                return;
            }
            if (!Utils.validateAddress(recipient)) {
                this.ui.showError('Please enter a valid recipient address.');
                return;
            }

            this.ui.hideMessages();
            this.ui.showMintingModal(recipient, amountAsTokens, isPrivateNote);
            this.ui.updateMintingTitle('PREPARING THE REQUEST');
            this.ui.updateProgressBar(0);

            const powData = await getPowChallenge(this.apiUrl, recipient, amount);
            const nonce = await this.findValidNonce(powData.challenge, powData.target);

            this.ui.updateMintingTitle('MINTING TOKENS');
            this.ui.updateProgressBar(33);

            const getTokensResponse = await getTokens(this.apiUrl, powData.challenge, nonce, recipient, amount, isPrivateNote);

            this.ui.updateMintingTitle('CONFIRMING TRANSACTION');
            this.ui.updateProgressBar(66);

            await this.pollNote(getTokensResponse.note_id);

            this.ui.showCompletedModal(
                recipient,
                amountAsTokens,
                isPrivateNote,
                getTokensResponse.tx_id,
                getTokensResponse.note_id,
                (noteId) => this.downloadNote(noteId),
                () => {
                    this.ui.hideModals();
                    this.ui.resetForm();
                }
            );
        } catch (error) {
            this.ui.showError(error);
            return;
        }
    }

    startMetadataPolling() {
        this.fetchMetadata();

        // Poll every 4 seconds
        this.metadataInterval = setInterval(() => {
            this.fetchMetadata();
        }, 4000);
    }

    async fetchMetadata() {
        try {
            const data = await getMetadata(this.apiUrl);

            this.ui.setIssuanceAndSupply(data.issuance, data.max_supply, data.decimals);
            this.powLoadDifficulty = data.pow_load_difficulty;
            this.baseAmount = data.base_amount;

            if (!this.metadataInitialized) {
                this.metadataInitialized = true;
                this.ui.setFaucetId(data.id);
                this.ui.setExplorerUrl(data.explorer_url);
                this.ui.setTokenOptions(this.tokenAmountOptions, data.decimals);
                this.updateTokenHint(this.tokenAmountOptions[0]);
            }
        } catch (error) {
            console.error('Error fetching metadata:', error);
        }
    }

    updateTokenHint(requestedAmount) {
        const estimatedTime = this.computePowTimeEstimation(requestedAmount, this.baseAmount, this.powLoadDifficulty);
        this.ui.setTokenHint(estimatedTime);
    }

    computePowTimeEstimation(requestedAmount, baseAmount, loadDifficulty) {
        const requestComplexity =
            Math.floor(requestedAmount / Number(baseAmount)) + 1;
        const difficulty = requestComplexity * Number(loadDifficulty);
        const difficultyBits = Math.log2(difficulty);

        let estimatedTime;
        if (difficultyBits <= 17) {
            estimatedTime = `<5s`;
        } else if (difficultyBits <= 18) {
            estimatedTime = `5-15s`;
        } else if (difficultyBits <= 19) {
            estimatedTime = `15-30s`;
        } else if (difficultyBits <= 20) {
            estimatedTime = `30s-1m`;
        } else if (difficultyBits <= 21) {
            estimatedTime = `1-5m`;
        } else {
            estimatedTime = `5m+`;
        }

        return estimatedTime;
    }

    async downloadNote(noteId) {
        this.ui.hidePrivateModalError();
        try {
            const data = await get_note(this.apiUrl, noteId);

            // Decode base64
            const binaryString = atob(data.data_base64);
            const byteArray = new Uint8Array(binaryString.length);
            for (let i = 0; i < binaryString.length; i++) {
                byteArray[i] = binaryString.charCodeAt(i);
            }

            const blob = new Blob([byteArray], { type: 'application/octet-stream' });
            Utils.downloadBlob(blob, 'note.mno');

            this.ui.showNoteDownloadedMessage();
        } catch (error) {
            console.error('Error downloading note:', error);
            this.ui.showPrivateModalError('Failed to download note: ' + error.message);
        }
    }

    pollNote(noteId) {
        return new Promise((resolve, reject) => {
            // Poll every 500ms for the first 10 seconds, then every 1s for the next 30 seconds, then every 5s.
            let currentInterval = 500;
            let pollInterval;
            let elapsedTime = 0;
            // Timeout after 5 minutes
            const timeout = 300000;
            let timeoutId;

            const poll = async () => {
                try {
                    const note = await this.rpcClient.getNotesById([NoteId.fromHex(noteId)]);
                    if (note && note.length > 0) {
                        clearInterval(pollInterval);
                        clearTimeout(timeoutId);
                        resolve();
                        return;
                    }

                    elapsedTime += currentInterval;

                    if (elapsedTime <= 10000) {
                        currentInterval = 500;
                    } else if (elapsedTime <= 40000) {
                        currentInterval = 1000;
                    } else {
                        currentInterval = 5000;
                    }

                    // Update the interval
                    clearInterval(pollInterval);
                    pollInterval = setInterval(poll, currentInterval);
                } catch (error) {
                    console.error('Error polling for note:', error);
                    clearInterval(pollInterval);
                    clearTimeout(timeoutId);
                    reject('Error fetching note confirmation.');
                }
            };
            pollInterval = setInterval(poll, currentInterval);
            timeoutId = setTimeout(() => {
                clearInterval(pollInterval);
                reject(new Error('Timeout while waiting for tx to be committed. Please try again later.'));
            }, timeout);
        });
    }

    async findValidNonce(challenge, target) {
        let nonce = 0;
        let targetNum = BigInt(target);
        const challengeBytes = Uint8Array.fromHex(challenge);

        while (true) {
            nonce = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);

            try {
                // Convert nonce to 8-byte big-endian format to match backend
                const nonceBytes = new ArrayBuffer(8);
                const nonceView = new DataView(nonceBytes);
                nonceView.setBigUint64(0, BigInt(nonce), false); // false = big-endian
                const nonceByteArray = new Uint8Array(nonceBytes);

                // Combine challenge and nonce
                const combined = new Uint8Array(challengeBytes.length + nonceByteArray.length);
                combined.set(challengeBytes);
                combined.set(nonceByteArray, challengeBytes.length);

                // Compute SHA-256 hash using Web Crypto API
                const hashBuffer = await window.crypto.subtle.digest('SHA-256', combined);
                const hashArray = new Uint8Array(hashBuffer);

                // Take the first 8 bytes of the hash and parse them as u64 in big-endian
                const first8Bytes = hashArray.slice(0, 8);
                const dataView = new DataView(first8Bytes.buffer);
                const digest = dataView.getBigUint64(0, false); // false = big-endian

                // Check if the hash is less than the target
                if (digest < targetNum) {
                    return nonce;
                }
            } catch (error) {
                console.error('Error computing hash:', error);
                throw new Error('Failed to compute hash: ' + error.message);
            }

            // Yield to browser to prevent freezing
            if (nonce % 1000 === 0) {
                await new Promise(resolve => setTimeout(resolve, 0));
            }
        }
    }
}
