class MidenFaucet {
    constructor() {
        this.recipientInput = document.getElementById('recipient-address');
        this.tokenSelect = document.getElementById('token-amount');
        this.privateButton = document.getElementById('send-private-button');
        this.publicButton = document.getElementById('send-public-button');
        this.successMessage = document.getElementById('success-message');
        this.errorMessage = document.getElementById('error-message');
        this.faucetAddress = document.getElementById('faucet-address');
        this.progressFill = document.getElementById('progress-fill');
        this.tokensClaimed = document.getElementById('tokens-claimed');
        this.tokensSupply = document.getElementById('tokens-supply');

        // Check if SHA3 is available right from the start
        if (typeof sha3_256 === 'undefined') {
            console.error("SHA3 library not loaded initially");
            this.showError('Cryptographic library not loaded. Please refresh the page.');
        }

        this.fetchMetadata();
        this.privateButton.addEventListener('click', () => this.handleSendTokens(true));
        this.publicButton.addEventListener('click', () => this.handleSendTokens(false));
    }

    async handleSendTokens(isPrivateNote) {
        const recipient = this.recipientInput.value.trim();
        const amount = this.tokenSelect.value;

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
        this.showMintingModal(recipient, amount, isPrivateNote);

        this.updateMintingTitle('PREPARING THE REQUEST');

        const powData = await this.getPowChallenge(recipient);
        if (!powData) {
            this.hideModals();
            return;
        }
        const nonce = await Utils.findValidNonce(powData.challenge, powData.target);

        this.updateMintingTitle('MINTING TOKENS');

        try {
            await this.getTokens(powData.challenge, nonce, recipient, amount, isPrivateNote);
            this.resetForm();
        } catch (error) {
            this.showError(`Failed to send tokens: ${error.message}`);
        }
    }

    async fetchMetadata() {
        fetch(window.location.origin + '/get_metadata')
            .then(response => response.json())
            .then(data => {
                this.faucetAddress.textContent = data.id;
                for (const amount of data.asset_amount_options) {
                    const option = document.createElement('option');
                    option.value = amount;
                    option.textContent = amount;
                    this.tokenSelect.appendChild(option);
                }
                // TODO: add metadata values
                this.tokensClaimed.textContent = '0';
                this.tokensSupply.textContent = '100,000,000,000';
                this.progressFill.style.width = '0%';
            })
            .catch(error => {
                console.error('Error fetching metadata:', error);
                this.showError('Failed to load metadata. Please try again.');
            });
    }

    async getPowChallenge(recipient) {
        let powResponse;
        try {
            powResponse = await fetch(window.location.origin + '/pow?' + new URLSearchParams({
                account_id: recipient
            }), {
                method: "GET"
            });
        } catch (error) {
            this.showError('Connection failed.');
            return;
        }

        if (!powResponse.ok) {
            const message = await powResponse.text();
            this.showError(message);
            return;
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
        const evtSource = new EventSource(window.location.origin + '/get_tokens?' + new URLSearchParams(params));

        evtSource.onerror = (_) => {
            // Either rate limit exceeded or invalid account id. The error event does not contain the reason.
            evtSource.close();
            this.showError('Please try again soon.');
            return;
        };

        evtSource.addEventListener("get-tokens-error", (event) => {
            console.error('EventSource failed:', event.data);
            evtSource.close();

            const data = JSON.parse(event.data);
            this.showError('Failed to receive tokens: ' + data.message);
            return;
        });

        evtSource.addEventListener("minted", (event) => {
            evtSource.close();
            this.setLoading(false);

            let data = JSON.parse(event.data);

            // TODO: this state should wait until note is committed - use web-client for this
            this.showCompletedModal(recipient, amount, isPrivateNote, data);
        });
    }

    async requestNote(noteId) {
        const response = await fetch(window.location.href + 'get_note?' + new URLSearchParams({
            note_id: noteId
        }));
        if (!response.ok) {
            this.showError('Failed to download note: ' + await response.text());
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
    }

    hideModals() {
        const mintingModal = document.getElementById('minting-modal');
        mintingModal.classList.remove('active');

        const completedPrivateModal = document.getElementById('completed-private-modal');
        completedPrivateModal.classList.remove('active');

        const completedPublicModal = document.getElementById('completed-public-modal');
        completedPublicModal.classList.remove('active');
    }

    showMintingModal(recipient, amount, isPrivateNote) {
        const modal = document.getElementById('minting-modal');
        const tokenAmount = document.getElementById('modal-token-amount');
        const recipientAddress = document.getElementById('modal-recipient-address');
        const noteType = document.getElementById('modal-note-type');

        // Update modal content
        tokenAmount.textContent = amount;
        recipientAddress.textContent = recipient;
        noteType.textContent = isPrivateNote ? 'PRIVATE' : 'PUBLIC';

        modal.classList.add('active');
    }

    showCompletedModal(recipient, amount, isPrivateNote, mintingData) {
        const mintingModal = document.getElementById('minting-modal');
        mintingModal.classList.remove('active');

        document.getElementById('completed-public-token-amount').textContent = amount;
        document.getElementById('completed-public-recipient-address').textContent = recipient;
        document.getElementById('completed-private-token-amount').textContent = amount;
        document.getElementById('completed-private-recipient-address').textContent = recipient;

        this.updateMintingTitle('TOKENS MINTED!');
        const completedPrivateModal = document.getElementById('completed-private-modal');
        const completedPublicModal = document.getElementById('completed-public-modal');

        if (isPrivateNote) {
            completedPrivateModal.classList.add('active');

            const downloadButton = document.getElementById('download-button');
            downloadButton.onclick = () => this.requestNote(mintingData.note_id);
        } else {
            completedPublicModal.classList.add('active');

            const explorerButton = document.getElementById('explorer-button');
            if (mintingData.explorer_url) {
                explorerButton.onclick = () => window.open(mintingData.explorer_url + '/tx/' + mintingData.transaction_id, '_blank');
            } else {
                explorerButton.onclick = () => this.showError('Explorer URL not available');
            }

        }

        // Add click anywhere to continue
        completedPublicModal.onclick = (_) => {
            this.hideModals();
            this.resetForm();
        }
        completedPrivateModal.onclick = (_) => {
            this.hideModals();
            this.resetForm();
        };
    }

    updateMintingTitle(title) {
        const mintingTitle = document.getElementById('minting-title');
        mintingTitle.textContent = title;
    }

    showError(message) {
        this.hideModals();
        this.errorMessage.textContent = message;
        this.errorMessage.style.display = 'block';
    }

    hideMessages() {
        this.errorMessage.style.display = 'none';
    }

    resetForm() {
        this.recipientInput.value = '';
    }
}

document.addEventListener('DOMContentLoaded', () => {
    new MidenFaucet();
});

const Utils = {
    validateAddress: (address) => {
        return /^(0x[0-9a-fA-F]{30}|[a-z]{1,4}1[a-z0-9]{32})$/i.test(address);
    },

    findValidNonce: async (challenge, target) => {
        // Check again if SHA3 is available
        if (typeof sha3_256 === 'undefined') {
            console.error("SHA3 library not properly loaded. SHA3 object:", sha3_256);
            throw new Error('SHA3 library not properly loaded. Please refresh the page.');
        }

        let nonce = 0;
        let targetNum = BigInt(target);

        while (true) {
            nonce = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);

            try {
                // Compute hash using SHA3 with the challenge and nonce
                let hash = sha3_256.create();
                hash.update(challenge); // Use the hex-encoded challenge string directly

                // Convert nonce to 8-byte big-endian format to match backend
                const nonceBytes = new ArrayBuffer(8);
                const nonceView = new DataView(nonceBytes);
                nonceView.setBigUint64(0, BigInt(nonce), false); // false = big-endian
                const nonceByteArray = new Uint8Array(nonceBytes);
                hash.update(nonceByteArray);

                // Take the first 8 bytes of the hash and parse them as u64 in big-endian
                let digest = BigInt("0x" + hash.hex().slice(0, 16));

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
    }
};
