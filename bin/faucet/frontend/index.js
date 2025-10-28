import { MidenWalletAdapter } from "@demox-labs/miden-wallet-adapter-miden";
import { PrivateDataPermission, WalletAdapterNetwork } from "@demox-labs/miden-wallet-adapter-base";
import { Endpoint, NoteId, RpcClient } from "@demox-labs/miden-sdk";

class MidenFaucet {
    constructor() {
        this.recipientInput = document.getElementById('recipient-address');
        this.tokenSelect = document.getElementById('token-amount');
        this.privateButton = document.getElementById('send-private-button');
        this.publicButton = document.getElementById('send-public-button');
        this.walletConnectButton = document.getElementById('wallet-connect-button');
        this.faucetAddress = document.getElementById('faucet-address');
        this.progressFill = document.getElementById('progress-fill');
        this.issuance = document.getElementById('issuance');
        this.tokensSupply = document.getElementById('tokens-supply');
        this.tokenAmountOptions = [100, 500, 1000];
        this.tokenAmountHint = document.getElementById('token-amount-hint');
        this.explorerUrl = null;
        this.metadataInitialized = false;
        this.metadata = null;

        // Check if Web Crypto API is available
        if (!window.crypto || !window.crypto.subtle) {
            console.error("Web Crypto API not available");
            this.showError('Web Crypto API not available. Please use a modern browser.');
        }
        this.privateButton.addEventListener('click', () => this.handleSendTokens(true));
        this.publicButton.addEventListener('click', () => this.handleSendTokens(false));
        this.walletConnectButton.addEventListener('click', () => this.handleWalletConnect());
        this.tokenSelect.addEventListener('change', () => this.updateTokenHint());

        this.walletAdapter = new MidenWalletAdapter({ appName: 'Miden Faucet' });

        this.init();
    }

    async init() {
        let config = await this.getConfigValues();
        this.apiUrl = config.api_url;
        this.rpcClient = new RpcClient(new Endpoint(config.node_url));
        this.startMetadataPolling();
    }

    async getConfigValues() {
        const response = await fetch('/config.json');
        if (!response.ok) {
            throw new Error(`Failed to fetch config.json file: ${response.status}`);
        }
        return JSON.parse(await response.json());
    }

    async handleWalletConnect() {
        try {
            await this.walletAdapter.connect(PrivateDataPermission.UponRequest, WalletAdapterNetwork.Testnet);

            if (this.walletAdapter.accountId) {
                this.recipientInput.value = this.walletAdapter.accountId;
            }
        } catch (error) {
            console.error("WalletConnectionError:", error);
            this.showError("Failed to connect wallet.");
        }
    }

    async handleSendTokens(isPrivateNote) {
        try {

            const recipient = this.recipientInput.value.trim();
            const amount = this.tokenSelect.value;
            const amountAsTokens = this.tokenSelect[this.tokenSelect.selectedIndex].textContent;

            if (!recipient) {
                this.showError('Recipient address is required.');
                return;
            }

            if (!amount || amount === '0') {
                this.showError('Amount is required.');
                return;
            }

            if (!Utils.validateAddress(recipient)) {
                this.showError('Please enter a valid recipient address.');
                return;
            }

            this.hideMessages();
            this.showMintingModal(recipient, amountAsTokens, isPrivateNote);
            this.updateProgressBar(0);

            this.updateMintingTitle('PREPARING THE REQUEST');

            const powData = await this.getPowChallenge(recipient, amount);
            const nonce = await Utils.findValidNonce(powData.challenge, powData.target);

            this.updateMintingTitle('MINTING TOKENS');
            this.updateProgressBar(33);

            const getTokensResponse = await this.getTokens(powData.challenge, nonce, recipient, amount, isPrivateNote);

            this.updateMintingTitle('CONFIRMING TRANSACTION');
            this.updateProgressBar(66);

            await this.pollNote(getTokensResponse.note_id);
            this.showCompletedModal(recipient, amountAsTokens, isPrivateNote, getTokensResponse.tx_id, getTokensResponse.note_id);
        } catch (error) {
            this.showError(error);
            return;
        }
    }

    startMetadataPolling() {
        this.fetchMetadata();

        // Poll every 2 seconds
        this.metadataInterval = setInterval(() => {
            this.fetchMetadata();
        }, 2000);
    }

    async fetchMetadata() {
        fetch(this.apiUrl + '/get_metadata')
            .then(response => response.json())
            .then(data => {
                this.metadata = data;
                this.faucetAddress.textContent = data.id;
                this.explorerUrl = data.explorer_url;

                if (!this.metadataInitialized) {
                    this.metadataInitialized = true;
                    this.tokenSelect.innerHTML = '';
                    for (const amount of this.tokenAmountOptions) {
                        const option = document.createElement('option');
                        const baseUnits = Utils.tokensToBaseUnits(amount, data.decimals);
                        option.value = baseUnits;
                        option.textContent = amount;
                        this.tokenSelect.appendChild(option);
                    }
                    this.updateTokenHint();
                }

                this.issuance.textContent = Utils.baseUnitsToTokens(data.issuance, data.decimals);
                this.tokensSupply.textContent = Utils.baseUnitsToTokens(data.max_supply, data.decimals);
                this.progressFill.style.width = (data.issuance / data.max_supply) * 100 + '%';
            })
            .catch(error => {
                console.error('Error fetching metadata:', error);
            });
    }

    async getPowChallenge(recipient, amount) {
        let powResponse;
        try {
            powResponse = await fetch(this.apiUrl + '/pow?' + new URLSearchParams({
                amount: amount,
                account_id: recipient
            }), {
                method: "GET"
            });
        } catch (error) {
            throw "Connection failed.";
        }

        if (!powResponse.ok) {
            const message = await powResponse.text();
            throw "Failed to get PoW challenge: " + message;
        }

        return await powResponse.json();
    }

    async getTokens(challenge, nonce, recipient, amount, isPrivateNote) {
        const params = {
            account_id: recipient,
            is_private_note: isPrivateNote,
            asset_amount: parseInt(amount),
            challenge: challenge,
            nonce: nonce
        };
        let response;
        try {
            response = await fetch(this.apiUrl + '/get_tokens?' + new URLSearchParams(params), {
                method: "GET"
            });
        } catch (error) {
            console.error(error);
            throw "Connection failed.";
        }

        if (!response.ok) {
            const message = await response.text();
            throw "Failed to receive tokens: " + message;
        }

        return response.json();
    }

    async downloadNote(noteId) {
        this.hidePrivateModalError();
        let response;
        try {
            response = await fetch(this.apiUrl + '/get_note?' + new URLSearchParams({
                note_id: noteId
            }));
        } catch (error) {
            this.showPrivateModalError('Connection failed.');
            return;
        }

        if (!response.ok) {
            this.showPrivateModalError('Failed to download note: ' + await response.text());
            return;
        }
        const data = await response.json();
        // Decode base64
        const binaryString = atob(data.data_base64);
        const byteArray = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
            byteArray[i] = binaryString.charCodeAt(i);
        }

        const blob = new Blob([byteArray], { type: 'application/octet-stream' });
        Utils.downloadBlob(blob, 'note.mno');

        this.showNoteDownloadedMessage();
    }

    showNoteDownloadedMessage() {
        const continueText = document.getElementById('private-continue-text');
        continueText.style.visibility = 'visible';
    }

    hideModals() {
        const mintingModal = document.getElementById('minting-modal');
        mintingModal.classList.remove('active');

        const completedPrivateModal = document.getElementById('completed-private-modal');
        completedPrivateModal.classList.remove('active');

        const completedPublicModal = document.getElementById('completed-public-modal');
        completedPublicModal.classList.remove('active');

        this.hideProgressBar();
    }

    showMintingModal(recipient, amountAsTokens, isPrivateNote) {
        const modal = document.getElementById('minting-modal');
        const tokenAmount = document.getElementById('modal-token-amount');
        const recipientAddress = document.getElementById('modal-recipient-address');
        const noteType = document.getElementById('modal-note-type');

        // Update modal content
        tokenAmount.textContent = amountAsTokens;
        recipientAddress.textContent = recipient;
        noteType.textContent = isPrivateNote ? 'PRIVATE' : 'PUBLIC';

        modal.classList.add('active');
    }

    showCompletedModal(recipient, amountAsTokens, isPrivateNote, txId, noteId) {
        const mintingModal = document.getElementById('minting-modal');
        mintingModal.classList.remove('active');

        document.getElementById('completed-public-token-amount').textContent = amountAsTokens;
        document.getElementById('completed-public-recipient-address').textContent = recipient;
        document.getElementById('completed-private-token-amount').textContent = amountAsTokens;
        document.getElementById('completed-private-recipient-address').textContent = recipient;

        this.updateMintingTitle('TOKENS MINTED!');
        const completedPrivateModal = document.getElementById('completed-private-modal');
        const completedPublicModal = document.getElementById('completed-public-modal');

        this.updateProgressBar(100);

        if (isPrivateNote) {
            completedPrivateModal.classList.add('active');
            this.setupDownloadButton(noteId);
        } else {
            completedPublicModal.classList.add('active');

            const explorerButton = document.getElementById('explorer-button');
            if (this.explorerUrl) {
                explorerButton.style.display = 'block';
                explorerButton.onclick = () => window.open(this.explorerUrl + 'tx/' + txId, '_blank');
            } else {
                explorerButton.style.display = 'none';
            }

            completedPublicModal.onclick = (e) => {
                const continueText = document.getElementById('public-continue-text');
                if (e.target === completedPublicModal || e.target === continueText) {
                    this.hideModals();
                    this.resetForm();
                }
            };
        }
    }

    updateMintingTitle(title) {
        const mintingTitle = document.getElementById('minting-title');
        mintingTitle.textContent = title;
    }

    showPublicModalError(message) {
        const publicModalError = document.getElementById('public-error-message');
        publicModalError.textContent = message;
        publicModalError.style.display = 'block';
    }

    showPrivateModalError(message) {
        const privateModalError = document.getElementById('private-error-message');
        privateModalError.textContent = message;
        privateModalError.style.display = 'block';
    }

    hidePrivateModalError() {
        const privateModalError = document.getElementById('private-error-message');
        privateModalError.style.display = 'none';
    }

    showError(message) {
        this.hideModals();
        const errorMessage = document.getElementById('error-message');
        errorMessage.textContent = message;
        errorMessage.style.display = 'block';
    }

    hideMessages() {
        const errorMessage = document.getElementById('error-message');
        errorMessage.style.display = 'none';

        const privateModalError = document.getElementById('private-error-message');
        privateModalError.style.display = 'none';

        const publicModalError = document.getElementById('public-error-message');
        publicModalError.style.display = 'none';

        const continueText = document.getElementById('private-continue-text');
        continueText.style.visibility = 'hidden';
    }

    resetForm() {
        this.recipientInput.value = '';
    }

    updateProgressBar(progress) {
        this.showProgressBar();
        const progressBarFill = document.getElementById('progress-bar-fill');
        progressBarFill.style.width = progress + '%';
    }

    showProgressBar() {
        const progressBarTotal = document.getElementById('progress-bar-total');
        progressBarTotal.classList.add('active');
    }

    hideProgressBar() {
        this.updateProgressBar(0);
        const progressBarTotal = document.getElementById('progress-bar-total');
        progressBarTotal.classList.remove('active');
    }

    updateTokenHint() {
        if (!this.metadata) return;

        const requestComplexity =
            Math.floor(this.tokenSelect.value / Number(this.metadata.base_amount)) + 1;
        const difficulty = requestComplexity * Number(this.metadata.pow_load_difficulty);
        const difficultyBits = Math.log2(difficulty);

        let estimatedTime;
        if (difficultyBits <= 18) {
            estimatedTime = `<5s`;
        } else if (difficultyBits <= 19) {
            estimatedTime = `5-15s`;
        } else if (difficultyBits <= 20) {
            estimatedTime = `15-30s`;
        } else if (difficultyBits <= 20.5) {
            estimatedTime = `30s-1m`;
        } else if (difficultyBits <= 21) {
            estimatedTime = `1-3m`;
        } else {
            estimatedTime = `3m+`;
        }
        this.tokenAmountHint.textContent = `Larger amounts take more time to mint. Estimated: ${estimatedTime}`;
    }

    showCloseButton() {
        const closeButton = document.getElementById('private-close-button');
        closeButton.style.display = 'block';
        closeButton.onclick = () => {
            closeButton.style.display = 'none';
            this.hideMessages();
            this.hideModals();
            this.resetForm();
        };
    }

    async setupDownloadButton(noteId) {
        const downloadButton = document.getElementById('download-button');
        downloadButton.onclick = async () => {
            await this.downloadNote(noteId);
            this.showCloseButton();
        };
    }

    pollNote(noteId) {
        return new Promise((resolve, reject) => {
            // Poll every 500ms for the first 10 seconds, then every 1s for the next 30 seconds, then every 5s.
            let currentInterval = 500;
            let pollInterval;
            let elapsedTime = 0;
            // Timeout after 5 minutes
            const timeout = 30000;
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
}

document.addEventListener('DOMContentLoaded', () => {
    new MidenFaucet();
});

const Utils = {
    validateAddress: (address) => {
        return /^(0x[0-9a-fA-F]{30}|[a-z]{1,4}1[a-z0-9]{35})$/i.test(address);
    },

    findValidNonce: async (challenge, target) => {
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
    },

    downloadBlob: (blob, filename) => {
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.style.display = 'none';
        a.href = url;
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        a.remove();
        window.URL.revokeObjectURL(url);
    },

    base64ToBlob: (base64) => {
        const binaryString = atob(base64);
        const byteArray = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
            byteArray[i] = binaryString.charCodeAt(i);
        }
        return new Blob([byteArray], { type: 'application/octet-stream' });
    },

    baseUnitsToTokens: (baseUnits, decimals) => {
        return (baseUnits / 10 ** decimals).toLocaleString();
    },

    tokensToBaseUnits: (tokens, decimals) => {
        return tokens * (10 ** decimals);
    },
};

// miden-sdk async import seems to interfere with
// this event, related mdn:
// https://developer.mozilla.org/en-US/docs/Web/API/Document/DOMContentLoaded_event#checking_whether_loading_is_already_complete
if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => {
        new MidenFaucet();
    });
} else {
    new MidenFaucet();
}
