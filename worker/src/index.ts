/**
 * Windows Whisper - Cloudflare Worker
 * 
 * Receives audio data and returns transcribed text using Whisper AI.
 */

interface Env {
    AI: Ai;
    API_KEY: string;
}

interface TranscribeRequest {
    audio: string; // Base64 encoded audio data
}

interface TranscribeResponse {
    success: boolean;
    text?: string;
    error?: string;
}

export default {
    async fetch(request: Request, env: Env): Promise<Response> {
        // CORS headers for the Tauri app
        const corsHeaders = {
            'Access-Control-Allow-Origin': '*',
            'Access-Control-Allow-Methods': 'POST, OPTIONS',
            'Access-Control-Allow-Headers': 'Content-Type, X-API-Key',
        };

        // Handle preflight requests
        if (request.method === 'OPTIONS') {
            return new Response(null, { headers: corsHeaders });
        }

        // Only accept POST requests to /transcribe
        const url = new URL(request.url);
        if (request.method !== 'POST' || url.pathname !== '/transcribe') {
            return new Response(
                JSON.stringify({ success: false, error: 'Not found' }),
                {
                    status: 404,
                    headers: { ...corsHeaders, 'Content-Type': 'application/json' }
                }
            );
        }

        // Validate API key
        const apiKey = request.headers.get('X-API-Key');
        if (!apiKey || apiKey !== env.API_KEY) {
            return new Response(
                JSON.stringify({ success: false, error: 'Unauthorized' }),
                {
                    status: 401,
                    headers: { ...corsHeaders, 'Content-Type': 'application/json' }
                }
            );
        }

        try {
            // Parse the request body
            const body = await request.json() as TranscribeRequest;

            if (!body.audio) {
                return new Response(
                    JSON.stringify({ success: false, error: 'No audio data provided' }),
                    {
                        status: 400,
                        headers: { ...corsHeaders, 'Content-Type': 'application/json' }
                    }
                );
            }

            // Decode base64 audio to Uint8Array
            const audioBytes = Uint8Array.from(atob(body.audio), c => c.charCodeAt(0));

            // Call Whisper AI model
            const result = await env.AI.run('@cf/openai/whisper', {
                audio: [...audioBytes],
            });

            const response: TranscribeResponse = {
                success: true,
                text: result.text || '',
            };

            return new Response(JSON.stringify(response), {
                headers: { ...corsHeaders, 'Content-Type': 'application/json' },
            });

        } catch (error) {
            console.error('Transcription error:', error);

            const response: TranscribeResponse = {
                success: false,
                error: error instanceof Error ? error.message : 'Unknown error occurred',
            };

            return new Response(JSON.stringify(response), {
                status: 500,
                headers: { ...corsHeaders, 'Content-Type': 'application/json' },
            });
        }
    },
};
