class MidenFaucet {
    constructor() {
        this.form = document.getElementById('faucetForm');
        this.recipientInput = document.getElementById('recipient-address');
        this.tokenSelect = document.getElementById('token-amount');
        this.privateBtn = document.getElementById('send-private-button');
        this.publicBtn = document.getElementById('send-public-button');
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
        this.privateBtn.addEventListener('click', () => this.handleSendTokens(true));
        this.publicBtn.addEventListener('click', () => this.handleSendTokens(false));
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

        this.setLoading(true);
        this.hideMessages();

        const powData = await this.getPowChallenge(recipient);
        const nonce = await Utils.findValidNonce(powData.challenge, powData.target);

        try {
            await this.getTokens(powData.challenge, nonce, recipient, amount, isPrivateNote);
            this.resetForm();
        } catch (error) {
            this.showError(`Failed to send tokens: ${error.message}`);
        } finally {
            this.setLoading(false);
        }
    }

    async fetchMetadata() {
        fetch(window.location.href + 'get_metadata')
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
            powResponse = await fetch(window.location.href + 'pow?' + new URLSearchParams({
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
        const evtSource = new EventSource(window.location.href + 'get_tokens?' + new URLSearchParams(params));

        evtSource.onopen = () => {
            this.showSuccess("Request on queue...");
        };

        evtSource.onerror = (_) => {
            // Either rate limit exceeded or invalid account id. The error event does not contain the reason.
            evtSource.close();
            this.showError('Please try again soon.');
            this.setLoading(false);
        };

        evtSource.addEventListener("get-tokens-error", (event) => {
            console.error('EventSource failed:', event.data);
            evtSource.close();

            const data = JSON.parse(event.data);
            this.showError('Failed to receive tokens: ' + data.message);
            this.setLoading(false);
        });

        evtSource.addEventListener("update", (event) => {
            this.showSuccess(event.data);
        });

        evtSource.addEventListener("minted", (event) => {
            evtSource.close();
            this.setLoading(false);

            let data = JSON.parse(event.data);

            // TODO: this is temporary, will be redesigned later
            this.showSuccess(`Created note ${data.note_id} for account ${data.account_id}. See transaction ${data.explorer_url + '/tx/' + data.transaction_id} on the explorer.`);
            if (isPrivateNote) {
                const blob = Utils.base64ToBlob(data.data_base64);
                Utils.downloadBlob(blob, 'note.mno');
            }
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
        downloadBlob(blob, 'note.mno');
    }

    setLoading(isLoading) {
        if (isLoading) {
            this.privateBtn.disabled = true;
            this.publicBtn.disabled = true;
            this.privateBtn.style.opacity = '0.6';
            this.publicBtn.style.opacity = '0.6';
        } else {
            this.privateBtn.disabled = true;
            this.publicBtn.disabled = true;
            this.privateBtn.style.opacity = '1';
            this.publicBtn.style.opacity = '1';
        }
    }

    showSuccess(message) {
        this.successMessage.textContent = message;
        this.successMessage.style.display = 'block';
        this.errorMessage.style.display = 'none';
    }

    showError(message) {
        this.errorMessage.textContent = message;
        this.errorMessage.style.display = 'block';
        this.successMessage.style.display = 'none';
    }

    hideMessages() {
        this.successMessage.style.display = 'none';
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
                hash.update(challenge);  // Use the hex-encoded challenge string directly

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
