export async function getConfig() {
    const response = await fetch('/config.json');
    if (!response.ok) {
        throw new Error(`Failed to fetch config.json file: ${response.statusText}`);
    }
    return JSON.parse(await response.json());
}

export async function getMetadata(backendUrl) {
    const response = await fetch(backendUrl + '/get_metadata');
    if (!response.ok) {
        throw new Error(`Failed to get metadata: ${response.statusText}`);
    }
    return response.json();
}

export async function getPowChallenge(backendUrl, recipient, amount) {
    const response = await fetch(backendUrl + '/pow?' + new URLSearchParams({
        amount: amount,
        account_id: recipient
    }));

    if (!response.ok) {
        const message = await response.text();
        throw new Error(`Failed to get PoW challenge: ${message}`);
    }

    return response.json();
}

export async function getTokens(backendUrl, challenge, nonce, recipient, amount, isPrivateNote) {
    const params = {
        account_id: recipient,
        is_private_note: isPrivateNote,
        asset_amount: parseInt(amount),
        challenge: challenge,
        nonce: nonce
    };

    const response = await fetch(backendUrl + '/get_tokens?' + new URLSearchParams(params));

    if (!response.ok) {
        const message = await response.text();
        throw new Error(`Failed to receive tokens: ${message}`);
    }

    return response.json();
}

export async function get_note(backendUrl, noteId) {
    const response = await fetch(backendUrl + '/get_note?' + new URLSearchParams({
        note_id: noteId
    }));
    if (!response.ok) {
        const message = await response.text();
        throw new Error(`Failed to get note: ${message}`);
    }
    return response.json();
}
