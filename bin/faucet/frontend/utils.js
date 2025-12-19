export const Utils = {
    validateAddress: (address) => {
        return /^(0x[0-9a-fA-F]{30}|[a-z]{1,4}1[a-z0-9]{32})(?:_[a-z0-9]+)?$/i.test(address);
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
        return (baseUnits / 10 ** decimals).toLocaleString(undefined, {
            maximumFractionDigits: 0,
        });
    },

    tokensToBaseUnits: (tokens, decimals) => {
        return tokens * (10 ** decimals);
    },

    idFromBech32: (address) => {
        return address.split('_')[0];
    },
};
