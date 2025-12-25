import { Utils } from "./utils";

export class UIController {
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
        this.tokenAmountHint = document.getElementById('token-amount-hint');
        this.explorerUrl = null;
    }

    setupEventListeners(onSendTokens, onWalletConnect, onTokenSelect) {
        this.privateButton.addEventListener('click', () => onSendTokens(true));
        this.publicButton.addEventListener('click', () => onSendTokens(false));
        this.walletConnectButton.addEventListener('click', onWalletConnect);
        this.tokenSelect.addEventListener('change', (event) => onTokenSelect(event.target.value));
    }

    getFormData() {
        return {
            recipient: this.recipientInput.value.trim(),
            amount: this.tokenSelect.value,
            amountAsTokens: this.tokenSelect[this.tokenSelect.selectedIndex].textContent
        };
    }

    setRecipientAddress(address) {
        this.recipientInput.value = address;
    }

    resetForm() {
        this.recipientInput.value = '';
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

    showCompletedModal(recipient, amountAsTokens, isPrivateNote, txId, noteId, onDownloadNote, onClose) {
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
            this.setupDownloadButton(noteId, onDownloadNote);
        } else {
            completedPublicModal.classList.add('active');

            const explorerButton = document.getElementById('explorer-button');
            if (this.explorerUrl) {
                explorerButton.style.display = 'block';
                explorerButton.onclick = () => window.open(`${this.explorerUrl}/tx/${txId}`, '_blank');
            } else {
                explorerButton.style.display = 'none';
            }

            completedPublicModal.onclick = (e) => {
                const continueText = document.getElementById('public-continue-text');
                if (e.target === completedPublicModal || e.target === continueText) {
                    onClose();
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

    setTokenHint(estimatedTime) {
        this.tokenAmountHint.textContent = `Larger amounts take more time to mint. Estimated: ${estimatedTime}`;
    }

    showNoteDownloadedMessage() {
        const continueText = document.getElementById('private-continue-text');
        continueText.textContent = 'YOUR NOTE HAS BEEN DOWNLOADED. CLICK X TO CONTINUE';
        continueText.style.visibility = 'visible';
    }

    showNoteImportedMessage() {
        const continueText = document.getElementById('private-continue-text');
        continueText.textContent = 'YOUR NOTE HAS BEEN IMPORTED TO YOUR WALLET. CLICK X TO CONTINUE';
        continueText.style.visibility = 'visible';
    }

    showCloseButton(onClose) {
        const closeButton = document.getElementById('private-close-button');
        closeButton.style.display = 'block';
        closeButton.onclick = () => {
            closeButton.style.display = 'none';
            this.hideMessages();
            this.hideModals();
            this.resetForm();
            onClose();
        };
    }

    setupDownloadButton(noteId, onDownloadNote) {
        const downloadButton = document.getElementById('download-button');
        downloadButton.onclick = async () => {
            await onDownloadNote(noteId);
            this.showCloseButton(() => { });
        };
    }

    setTokenOptions(tokenAmountOptions, decimals) {
        this.tokenSelect.innerHTML = '';
        for (const amount of tokenAmountOptions) {
            const option = document.createElement('option');
            const baseUnits = Utils.tokensToBaseUnits(amount, decimals);
            option.value = baseUnits;
            option.textContent = amount;
            this.tokenSelect.appendChild(option);
        }
    }

    setFaucetId(id) {
        this.faucetAddress.textContent = id;
    }

    setExplorerUrl(url) {
        this.explorerUrl = url;
    }

    setIssuanceAndSupply(issuance, max_supply, decimals) {
        this.issuance.textContent = Utils.baseUnitsToTokens(issuance, decimals);
        this.tokensSupply.textContent = Utils.baseUnitsToTokens(max_supply, decimals);
        this.progressFill.style.width = (issuance / max_supply) * 100 + '%';
    }
}
