$ErrorActionPreference = "Stop"

Write-Host "🚀 Starting Windows Whisper Worker Deployment..." -ForegroundColor Cyan

# Navigate to worker directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$WorkerDir = Join-Path $ScriptDir "worker"

if (!(Test-Path $WorkerDir)) {
    Write-Error "❌ Worker directory not found at $WorkerDir"
    exit 1
}

Set-Location $WorkerDir

# Install dependencies
Write-Host "`n📦 Installing dependencies..." -ForegroundColor Yellow
npm install
if ($LASTEXITCODE -ne 0) {
    Write-Error "❌ Failed to install dependencies"
    exit 1
}

# Login check (basic check)
Write-Host "`n🔑 Checking Cloudflare login status..." -ForegroundColor Yellow
try {
    $whoami = npx wrangler whoami 2>&1
    if ($whoami -match "You are not authenticated") {
        Write-Host "⚠️ You are not logged in to Cloudflare." -ForegroundColor Red
        Write-Host "👉 A browser window will open for you to login." -ForegroundColor Cyan
        npx wrangler login
    }
} catch {
    # Ignore error, deploy will fail if not logged in
}

# DeployThank you.
Write-Host "`n☁️ Deploying Worker..." -ForegroundColor Yellow
npx wrangler deploy
if ($LASTEXITCODE -ne 0) {
    Write-Error "❌ Deployment failed"
    exit 1
}

Write-Host "`n✅ Worker deployed successfully!" -ForegroundColor Green

# API Key Setup
$response = Read-Host "`n🔐 Do you want to set/update your API Key now? (y/n)"
if ($response -eq 'y') {
    Write-Host "👉 Enter your secret API key when prompted below:" -ForegroundColor Cyan
    npx wrangler secret put API_KEY
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ API Key set successfully!" -ForegroundColor Green
    }
}

Write-Host "`n🎉 Setup Complete!" -ForegroundColor Cyan
Write-Host "Don't forget to update your Desktop App Settings with the Worker URL and API Key." -ForegroundColor Gray
