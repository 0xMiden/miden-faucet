import { Utils } from "./utils";

export class UIController {
    constructor() {
        this.recipientInput = document.getElementById('recipient-address');
        this.tokenSelect = document.getElementById('token-amount');
        this.privateButton = document.getElementById('send-private-button');
        this.publicButton = document.getElementById('send-public-button');
        this.walletConnectButton = document.getElementById('wallet-connect-button');
        this.faucetAddress = document.getElementById('faucet-address');
        this.issuanceFill = document.getElementById('issuance-fill');
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
    }

    showMintingModal(recipient, amountAsTokens, isPrivateNote) {
        const modal = document.getElementById('minting-modal');
        const tokenAmount = document.getElementById('modal-token-amount');
        const recipientAddress = document.getElementById('modal-recipient-address');
        const noteType = document.getElementById('modal-note-type');

        // Update modal content
        tokenAmount.textContent = amountAsTokens;
        recipientAddress.textContent = recipient;
        noteType.textContent = isPrivateNote ? 'Private' : 'Public';

        modal.classList.add('active');
    }

    setMintingTitle(title) {
        const mintingTitle = document.getElementById('minting-title');
        mintingTitle.textContent = title;
    }

    setMintingHint(hint) {
        const mintingHint = document.getElementById('minting-hint');
        mintingHint.textContent = hint;
    }

    hideMintingModal() {
        const mintingModal = document.getElementById('minting-modal');
        mintingModal.classList.remove('active');
    }

    showCompletedPrivateModal(recipient, amountAsTokens, noteId, txId, onDownloadNote) {
        document.getElementById('completed-private-token-amount').textContent = amountAsTokens;
        document.getElementById('completed-private-recipient-address').textContent = recipient;
        const completedPrivateModal = document.getElementById('completed-private-modal');
        completedPrivateModal.classList.add('active');
        this.setupDownloadButton(noteId, onDownloadNote);
        const privateExplorerButton = document.getElementById('private-explorer-button');
        this.setupExplorerButton(privateExplorerButton, txId);
    }

    setupExplorerButton(explorerButton, txId) {
        if (this.explorerUrl) {
            explorerButton.style.display = 'block';
            explorerButton.onclick = () => window.open(`${this.explorerUrl}/tx/${txId}`, '_blank');
        } else {
            explorerButton.style.display = 'none';
        }
    }

    showCompletedPublicModal(recipient, amountAsTokens, txId) {
        document.getElementById('completed-public-token-amount').textContent = amountAsTokens;
        document.getElementById('completed-public-recipient-address').textContent = recipient;
        const completedPublicModal = document.getElementById('completed-public-modal');
        completedPublicModal.classList.add('active');

        const publicExplorerButton = document.getElementById('public-explorer-button');
        this.setupExplorerButton(publicExplorerButton, txId);
        completedPublicModal.onclick = (e) => {
            if (e.target !== publicExplorerButton) {
                this.hideModals();
                this.resetForm();
            }
        };
    }

    showRequestFailedError(title, description) {
        this.showError(title, description);

        const icon = document.getElementById('error-icon');
        icon.style.display = 'block';
    }

    showConnectionError(title, description) {
        this.showError(title, description);

        const icon = document.getElementById('warning-icon');
        icon.style.display = 'block';
    }

    showInvalidRequestError(title, description) {
        this.showError(title, description);

        const icon = document.getElementById('invalid-icon');
        icon.style.display = 'block';
    }

    showWaitError(title, description) {
        this.showError(title, description);

        const icon = document.getElementById('wait-error-icon');
        icon.style.display = 'block';
    }

    showStillLoading(title, description) {
        this.showError(title, description);

        const icon = document.getElementById('wait-icon');
        icon.style.display = 'block';

        const errorMessage = document.getElementById('home-error-message');
        errorMessage.style.backgroundColor = '#F6DED2';
    }

    hideIcons() {
        const warningIcon = document.getElementById('warning-icon');
        warningIcon.style.display = 'none';

        const waitErrorIcon = document.getElementById('wait-error-icon');
        waitErrorIcon.style.display = 'none';

        const waitIcon = document.getElementById('wait-icon');
        waitIcon.style.display = 'none';

        const invalidIcon = document.getElementById('invalid-icon');
        invalidIcon.style.display = 'none';

        const errorIcon = document.getElementById('error-icon');
        errorIcon.style.display = 'none';
    }

    showError(title, description) {
        this.hideIcons();
        this.hideNoteHints();

        const errorTitle = document.getElementById('home-error-message-title');
        errorTitle.textContent = title;

        const errorDescription = document.getElementById('home-error-message-description');
        errorDescription.textContent = description;

        const errorMessage = document.getElementById('home-error-message');
        errorMessage.style.display = 'flex';
    }

    hideErrors() {
        this.hideIcons();

        const errorMessage = document.getElementById('home-error-message');
        errorMessage.style.display = 'none';
        errorMessage.style.backgroundColor = '#FFE8E9';

    }

    setTokenHint(estimatedTime) {
        this.tokenAmountHint.textContent = `Larger amounts take more time to mint. Estimated: ${estimatedTime}`;
    }

    showCloseButton() {
        const closeButton = document.getElementById('private-close-button');
        closeButton.style.display = 'block';
        closeButton.onclick = () => {
            closeButton.style.display = 'none';
            this.hideErrors();
            this.hideModals();
            this.resetForm();
            const downloadButton = document.getElementById('download-button');
            downloadButton.classList.remove('pressed')
            this.hideNoteHints();
        };
    }

    setupDownloadButton(noteId, onDownloadNote) {
        const downloadButton = document.getElementById('download-button');
        document.getElementById('download-button-text').textContent = 'Download Note';
        downloadButton.onclick = async () => {
            this.hideErrors();
            downloadButton.classList.add('pressed');
            document.getElementById('download-button-text').textContent = 'Download Again';
            this.showCloseButton();

            await onDownloadNote(noteId);
        };
    }

    showDownloadedNoteHints() {
        const nextSteps = document.getElementById('next-steps');
        nextSteps.style.display = 'block';
        const nextStepsList = document.getElementById('next-steps-list');
        nextStepsList.display = 'block';
        const warningText = document.getElementById('warning-text');
        warningText.style.display = 'block';
    }

    showNoteSentHints() {
        const nextSteps = document.getElementById('next-steps');
        nextSteps.style.display = 'block';

        const step = document.getElementById('single-next-step');
        step.textContent = 'Note has been sent to your client, sync and consume'; //TODO: write properly
        // 1. sync 2. consume. if note does not appear, download an import manually
    }

    showNoteImportedHints() {
        const nextSteps = document.getElementById('next-steps');
        nextSteps.style.display = 'block';

        const step = document.getElementById('single-next-step');
        step.textContent = 'Note has been imported to your wallet, it\'s ready to claim'; //TODO: write properly
    }

    hideNoteHints() {
        const nextSteps = document.getElementById('next-steps');
        nextSteps.style.display = 'none';
        const warningText = document.getElementById('warning-text');
        warningText.style.display = 'none';

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
        this.issuanceFill.style.width = (issuance / max_supply) * 100 + '%';
    }
}
