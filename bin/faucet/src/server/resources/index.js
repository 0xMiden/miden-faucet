class MidenFaucet {
    constructor() {
        this.form = document.getElementById('faucetForm');
        this.recipientInput = document.getElementById('recipientAddress');
        this.tokenSelect = document.getElementById('tokenAmount');
        this.privateBtn = document.getElementById('sendPrivateBtn');
        this.publicBtn = document.getElementById('sendPublicBtn');
        this.successMessage = document.getElementById('successMessage');
        this.errorMessage = document.getElementById('errorMessage');

        this.initEventListeners();
        this.validateForm(); // Initial validation
    }

    initEventListeners() {
        this.privateBtn.addEventListener('click', () => this.handleSendTokens('private'));
        this.publicBtn.addEventListener('click', () => this.handleSendTokens('public'));

        this.recipientInput.addEventListener('input', () => this.validateForm());
        this.tokenSelect.addEventListener('change', () => this.validateForm());

        // Prevent form submission
        this.form.addEventListener('submit', (e) => e.preventDefault());
    }

    validateForm() {
        const isValid = this.recipientInput.value.trim() && this.tokenSelect.value;
        this.privateBtn.disabled = !isValid;
        this.publicBtn.disabled = !isValid;
    }

    async handleSendTokens(type) {
        const recipient = this.recipientInput.value.trim();
        const amount = this.tokenSelect.value;

        if (!recipient || !amount) {
            this.showError('Please fill in all required fields');
            return;
        }

        if (!this.isValidAddress(recipient)) {
            this.showError('Please enter a valid recipient address');
            return;
        }

        this.setLoading(type, true);
        this.hideMessages();

        try {
            await this.simulateTokenSend(recipient, amount, type);
            this.showSuccess(`Successfully sent ${amount} tokens to ${this.truncateAddress(recipient)} via ${type} note`);
            this.resetForm();
        } catch (error) {
            this.showError(`Failed to send tokens: ${error.message}`);
        } finally {
            this.setLoading(type, false);
        }
    }

    async simulateTokenSend(recipient, amount, type) {
        // Simulate network delay
        await new Promise(resolve => setTimeout(resolve, 1500));

        // Simulate random failure (10% chance)
        if (Math.random() < 0.1) {
            throw new Error('Network timeout');
        }

        // Simulate successful response
        return {
            success: true,
            txHash: '0x' + Math.random().toString(16).substr(2, 64),
            recipient,
            amount,
            type,
            timestamp: new Date().toISOString()
        };
    }

    setLoading(type, isLoading) {
        const btn = type === 'private' ? this.privateBtn : this.publicBtn;
        const originalContent = btn.innerHTML;

        if (isLoading) {
            btn.disabled = true;
            btn.style.opacity = '0.6';

            // Store original content and show loading state
            btn.dataset.originalContent = originalContent;
            btn.innerHTML = originalContent.replace(/Send/, 'Sending...');
        } else {
            btn.disabled = false;
            btn.style.opacity = '1';

            // Restore original content
            if (btn.dataset.originalContent) {
                btn.innerHTML = btn.dataset.originalContent;
                delete btn.dataset.originalContent;
            }

            // Re-validate form
            this.validateForm();
        }
    }

    showSuccess(message) {
        this.successMessage.textContent = message;
        this.successMessage.style.display = 'block';
        this.errorMessage.style.display = 'none';

        // Auto-hide after 5 seconds
        setTimeout(() => {
            this.hideMessages();
        }, 5000);
    }

    showError(message) {
        this.errorMessage.textContent = message;
        this.errorMessage.style.display = 'block';
        this.successMessage.style.display = 'none';

        // Auto-hide after 5 seconds
        setTimeout(() => {
            this.hideMessages();
        }, 5000);
    }

    hideMessages() {
        this.successMessage.style.display = 'none';
        this.errorMessage.style.display = 'none';
    }

    resetForm() {
        this.recipientInput.value = '';
        this.tokenSelect.value = '';
        this.validateForm();
    }

    isValidAddress(address) {
        // Basic validation for hex address (Ethereum-style)
        return /^0x[a-fA-F0-9]{40}$/.test(address);
    }

    truncateAddress(address) {
        if (address.length <= 10) return address;
        return `${address.slice(0, 6)}...${address.slice(-4)}`;
    }
}

// Initialize the application when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    new MidenFaucet();
});

// Add some utility functions for potential future use
const Utils = {
    formatNumber: (num) => {
        return new Intl.NumberFormat().format(num);
    },

    copyToClipboard: async (text) => {
        try {
            await navigator.clipboard.writeText(text);
            return true;
        } catch (err) {
            console.error('Failed to copy text: ', err);
            return false;
        }
    },

    validateTokenAmount: (amount) => {
        const num = parseInt(amount);
        return !isNaN(num) && num > 0 && num <= 10000;
    }
};
