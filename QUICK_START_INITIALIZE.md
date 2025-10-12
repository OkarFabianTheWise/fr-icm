# ICM Program Quick Start Guide

## Problem: "Program state does not exist or cannot be fetched: Account not found"

This error means the ICM program hasn't been initialized yet. Follow these steps to fix it.

## Step-by-Step Solution

### 1. Check Program Status

First, verify if the program is initialized:

```bash
curl "http://localhost:3000/api/v1/program/status?usdc_mint=4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
```

### 2. Login and Get JWT Token

You need to be authenticated to initialize the program:

```bash
# Register/Login to get JWT token
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "your-email@example.com",
    "password": "your-password"
  }'
```

Save the JWT token from the response.

### 3. Initialize the Program (One-Time Setup)

```bash
curl -X POST http://localhost:3000/api/v1/program/initialize \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN_HERE" \
  -d '{
    "fee_rate_bps": 500,
    "usdc_mint": "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
  }'
```

### 4. Create Your Profile

```bash
curl -X POST http://localhost:3000/api/v1/profile/create \
  -H "Authorization: Bearer YOUR_JWT_TOKEN_HERE"
```

### 5. Now You Can Use Other Endpoints

After initialization, you can:

- Create buckets
- Contribute to buckets
- Start trading
- Execute swaps

## Configuration Notes

- **fee_rate_bps**: Fee rate in basis points (500 = 5%)
- **usdc_mint**: Use the correct USDC mint for your network:
  - Mainnet: `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`
  - Devnet: `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`

## Verification

Check program status again to confirm initialization:

```bash
curl "http://localhost:3000/api/v1/program/status?usdc_mint=4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
```

Expected response:

```json
{
  "status": "success",
  "data": {
    "initialized": true,
    "message": "Program is initialized and ready to use"
  }
}
```

## Troubleshooting

- **Authentication Error**: Make sure JWT token is valid and in Authorization header
- **Invalid USDC Mint**: Use the correct mint address for your Solana network
- **Permission Error**: Ensure the authenticated user has admin privileges for initialization
