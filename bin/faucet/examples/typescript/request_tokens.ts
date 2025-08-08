import { sha3_256 } from '@noble/hashes/sha3';
import fs from 'fs';

async function sendPowRequest(baseUrl: string, accountId: string) {
    const powUrl = new URL('/pow', baseUrl);
    powUrl.searchParams.set('account_id', accountId);
    const powResp = await fetch(powUrl);
    if (!powResp.ok) throw new Error(`PoW error: ${powResp.status} ${await powResp.text()}`);
    const powJson: any = await powResp.json();
    const challenge: string = powJson.challenge;
    const target: bigint = BigInt(powJson.target);
    return { challenge, target };
}

async function solveChallenge(challenge: string, target: bigint) {
    let nonce = 0;
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
            const hashBytes: Uint8Array = hash.digest().slice(0, 8);
            let digest = BigInt('0x' + Array.from(hashBytes).map(b => b.toString(16).padStart(2, '0')).join(''));

            // Check if the hash is less than the target
            if (digest < target) {
                return nonce;
            }
        } catch (error: any) {
            console.error('Error computing hash:', error);
            throw new Error('Failed to compute hash: ' + error.message);
        }
    }
}


async function getTokens(baseUrl: string, account_id: string, nonce: number, challenge: string) {
    const params = new URLSearchParams({
        account_id: account_id,
        is_private_note: 'true',
        asset_amount: '100',
        challenge: challenge,
        nonce: nonce.toString()
    });

    const response = await fetch(`${baseUrl}/get_tokens?${params}`);
    if (!response.ok) throw new Error(`Get tokens error: ${response.status} ${await response.text()}`);

    const text = await response.text();
    const json = JSON.parse(text);
    const noteId = json.note_id;
    const txId = json.tx_id;
    const explorerUrl = json.explorer_url;
    return { noteId, txId, explorerUrl };
}

async function downloadNote(baseUrl: string, noteId: string) {
    const url = `${baseUrl}/get_note?note_id=${noteId}`;
    const response = await fetch(url);
    if (!response.ok) throw new Error(`Get note error: ${response.status} ${await response.text()}`);

    const text = await response.text();
    const json = JSON.parse(text);

    // Decode note with base64
    const noteData = Buffer.from(json.data_base64, 'base64');

    fs.writeFileSync('note.mno', noteData);
}

async function main() {
    const baseUrl = 'http://localhost:8080';
    const accountId = 'mlcl1qq8mcy8pdvl0cgqfkjzf8efjjsnlzf7q';

    let { challenge, target } = await sendPowRequest(baseUrl, accountId);
    let nonce = await solveChallenge(challenge, target);
    let { noteId, txId, explorerUrl } = await getTokens(baseUrl, accountId, nonce, challenge);
    console.log('Note ID:', noteId);
    console.log('Tx ID:', txId);
    console.log('Explorer URL:', explorerUrl);
    await downloadNote(baseUrl, noteId);
}

main();
