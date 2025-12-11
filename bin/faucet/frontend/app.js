import { MidenWalletAdapter } from "@demox-labs/miden-wallet-adapter-miden";
import { PrivateDataPermission, WalletAdapterNetwork } from "@demox-labs/miden-wallet-adapter-base";
import { Utils } from './utils.js';
import { UIController } from './ui.js';
import { getConfig, getMetadata, getPowChallenge, getTokens, get_note } from "./api.js";

export class MidenFaucetApp {
    constructor() {
        this.ui = new UIController();
        this.tokenAmountOptions = [100, 500, 1000];
        this.metadataInitialized = false;
        this.apiUrl = null;
        this.baseAmount = null;
        this.powLoadDifficulty = null;

        // Check if Web Crypto API is available
        if (!window.crypto || !window.crypto.subtle) {
            console.error("Web Crypto API not available");
            this.ui.showConnectionError('Web Crypto API not available', 'Please use a modern browser');
            return;
        }

        this.walletAdapter = new MidenWalletAdapter({ appName: 'Miden Faucet' });

        this.init();
    }

    async init() {
        try {
            let config = await getConfig();
            this.apiUrl = config.api_url;
            this.setupEventListeners();
            this.startMetadataPolling();
        } catch (error) {
            console.error('Failed to initialize app:', error);
            this.handleApiError(error, 'Connection failed', 'Some data couldn\'t be loaded right now.');
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

            if (this.walletAdapter.address) {
                this.ui.setRecipientAddress(this.walletAdapter.address);
            }
        } catch (error) {
            console.error("WalletConnectionError:", error);
            this.ui.showConnectionError("Connection failed", "Failed to connect wallet.");
        }
    }

    async handleSendTokens(isPrivateNote) {
        try {
            const { recipient, amount, amountAsTokens } = this.ui.getFormData();

            if (!recipient) {
                this.ui.showInvalidRequestError('Invalid address', 'Please enter a recipient address.');
                return;
            }
            if (!amount || amount === '0') {
                this.ui.showInvalidRequestError('Invalid amount', 'Please enter a non zero amount.');
                return;
            }
            if (!Utils.validateAddress(recipient)) {
                this.ui.showInvalidRequestError('Invalid address', 'Please enter a valid recipient address.');
                return;
            }

            this.ui.hideErrors();
            this.ui.showMintingModal(recipient, amountAsTokens, isPrivateNote);

            const powData = await getPowChallenge(this.apiUrl, recipient, amount);
            const nonce = await this.findValidNonce(powData.challenge, powData.target);

            const getTokensResponse = await getTokens(this.apiUrl, powData.challenge, nonce, recipient, amount, isPrivateNote);

            this.ui.hideMintingModal();
            if (isPrivateNote) {
                this.ui.showCompletedPrivateModal(recipient, amountAsTokens, getTokensResponse.note_id, getTokensResponse.tx_id, (noteId) => this.downloadNote(noteId));
            } else {
                this.ui.showCompletedPublicModal(recipient, amountAsTokens, getTokensResponse.tx_id, () => {
                    this.ui.hideModals();
                    this.ui.resetForm();
                });
            }
        } catch (error) {
            this.ui.hideMintingModal();
            this.handleApiError(error, 'Request failed', error.message);
            return;
        }
    }

    startMetadataPolling() {
        try {
            this.fetchMetadata();
        } catch (error) {
            this.ui.showConnectionError('Connection failed', 'Some data couldn\'t be loaded right now.');
            console.error('Error fetching metadata:', error);
        }

        // Poll every 4 seconds
        this.metadataInterval = setInterval(() => {
            try {
                this.fetchMetadata();
            } catch (error) {
                console.error('Error fetching metadata:', error);
            }
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
        } catch (error) {
            console.error('Error downloading note:', error);
            this.handleApiError(error, 'Download failed', error.message);
        }
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

    handleApiError(error, defaultTitle, defaultMessage) {
        // Check if it's an ApiError with a status code
        if (error.statusCode) {
            const statusCode = error.statusCode;
            const errorMessage = error.message || defaultMessage;

            switch (statusCode) {
                case 400: // Bad Request
                    this.ui.showInvalidRequestError('Invalid request', errorMessage);
                    break;
                case 429: // Too Many Requests (Rate Limited)
                    this.ui.showWaitError('Error!', errorMessage || 'Too many requests.');
                    break;
                case 500: // Internal Server Error
                    this.ui.showConnectionError('Server error', errorMessage || 'An internal server error occurred.');
                    break;
                case 503: // Service Unavailable
                    this.ui.showConnectionError('Service unavailable', errorMessage || 'The faucet is currently unavailable.');
                    break;
                default:
                    // For other status codes, use the default error handler
                    this.ui.showRequestFailedError(defaultTitle, errorMessage);
            }
        } else {
            // For non-API errors (e.g. network errors), use default handler
            this.ui.showRequestFailedError(defaultTitle, defaultMessage);
        }
    }
}
