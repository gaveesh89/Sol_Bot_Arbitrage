# Security Best Practices

## 🔐 Critical Security Guidelines

### Never Commit Private Keys
**NEVER** hardcode private keys, keypairs, or secrets directly in your code. This is the most critical security rule.

❌ **WRONG:**
```rust
let keypair = Keypair::from_bytes(&[1, 2, 3, ...]); // NEVER DO THIS
let private_key = "5J3mBbAH58CpQ3Y2..."; // NEVER DO THIS
```

✅ **CORRECT:**
```rust
// Load from secure environment variable
let private_key = std::env::var("WALLET_PRIVATE_KEY")
    .expect("WALLET_PRIVATE_KEY must be set");

// Or load from secure file (not in repository)
let keypair = read_keypair_file(&keypair_path)
    .expect("Failed to load keypair");
```

---

## 🛡️ Environment Variables

### Use Environment Variables for All Secrets
Store sensitive data in `.env` files that are **never** committed to version control.

**Required Environment Variables:**
- `WALLET_PRIVATE_KEY` or `WALLET_KEYPAIR_PATH` - Your wallet credentials
- `RPC_URL` - May contain API keys
- `RPC_WS_URL` - May contain API keys

**Setup:**
```bash
# Copy example and fill with your secrets
cp .env.example .env

# Edit .env with your actual values
nano .env  # or vim, code, etc.
```

**Load Environment Variables:**
```bash
# In your shell startup file (~/.zshrc, ~/.bashrc)
export $(cat .env | xargs)

# Or use direnv for automatic loading
# Install: brew install direnv
echo 'eval "$(direnv hook zsh)"' >> ~/.zshrc
direnv allow .
```

---

## 📁 Secure File Storage

### Keypair File Location
Store keypair files **outside** your repository, or ensure they are properly `.gitignore`d.

**Recommended Locations:**
```bash
# Option 1: User home directory
~/.config/solana/id.json
~/.config/solana/wallet.json

# Option 2: Secure system directory (Unix/Linux)
/etc/solana/keypairs/wallet.json

# Option 3: Project directory (MUST be in .gitignore)
./wallet.json  # Already in .gitignore
```

**File Permissions:**
```bash
# Set restrictive permissions (owner read/write only)
chmod 600 ~/.config/solana/id.json
chmod 600 ./wallet.json

# Verify permissions
ls -la ~/.config/solana/id.json
# Should show: -rw------- (600)
```

---

## 🚫 .gitignore Configuration

The following patterns are automatically ignored to prevent accidental commits:

### Keypairs and Private Keys
```
*.json          # All JSON files (except package.json, idl.json)
*.key           # Key files
*.pem           # PEM certificate files
wallet.json     # Specific wallet files
keypair.json
id.json
authority.json
```

### Environment Files
```
.env            # Main environment file
.env.local      # Local overrides
.env.*          # Any environment variant
.env.production
.env.development
```

### API Keys and Tokens
```
*_token.txt     # Token files
*_api_key.txt   # API key files
secrets/        # Secrets directory
credentials/    # Credentials directory
```

**Verify .gitignore is working:**
```bash
# Check what would be committed
git status

# Verify specific file is ignored
git check-ignore -v wallet.json
# Should output: .gitignore:32:wallet.json    wallet.json
```

---

## 🔑 Wallet Configuration

### Two Methods for Wallet Setup

#### Method 1: Keypair File (Recommended for Development)
```bash
# Generate new keypair
solana-keygen new --outfile ./wallet.json

# Set in .env
WALLET_KEYPAIR_PATH=./wallet.json
```

#### Method 2: Base58 Private Key (Alternative)
```bash
# Get private key from existing keypair
solana-keygen pubkey ./wallet.json  # Get public key
# Then extract private key (use with caution!)

# Set in .env
WALLET_PRIVATE_KEY=your_base58_encoded_private_key_here
```

**In Code (src/main.rs):**
```rust
// Priority: Keypair file first, then private key
if let Some(ref path) = wallet_config.keypair_path {
    read_keypair_file(path)?
} else if let Some(ref private_key) = wallet_config.private_key {
    Keypair::from_base58_string(private_key)
} else {
    anyhow::bail!("No wallet configured")
}
```

---

## 🌐 RPC Endpoint Security

### Protect API Keys in RPC URLs

Many RPC providers embed API keys in URLs. **Never** commit these.

**Bad Practice:**
```rust
// Hardcoded API key in code
let rpc_url = "https://rpc.helius.xyz/?api-key=abc123..."; // WRONG!
```

**Good Practice:**
```bash
# In .env file
RPC_URL=https://rpc.helius.xyz/?api-key=YOUR_ACTUAL_KEY_HERE
RPC_WS_URL=wss://rpc.helius.xyz/?api-key=YOUR_ACTUAL_KEY_HERE
```

```rust
// In code - load from environment
let rpc_url = std::env::var("RPC_URL")?;
```

---

## 🧪 Testing Security

### Never Use Real Keys in Tests

**For Unit Tests:**
```rust
#[cfg(test)]
mod tests {
    use solana_sdk::signature::Keypair;

    #[test]
    fn test_transaction() {
        // Generate ephemeral keypair for testing
        let test_keypair = Keypair::new();
        
        // Use test keypair (never committed)
        // ...
    }
}
```

**For Integration Tests (Devnet/Testnet):**
```bash
# Create separate test wallet
solana-keygen new --outfile ./test-wallet.json

# Fund from faucet
solana airdrop 2 --url devnet --keypair ./test-wallet.json

# Use in tests
WALLET_KEYPAIR_PATH=./test-wallet.json cargo test
```

---

## 🔍 Security Checklist

Before committing code, verify:

- [ ] No private keys in source files
- [ ] No API keys in source files
- [ ] `.env` is in `.gitignore`
- [ ] Keypair files are in `.gitignore`
- [ ] Wallet files have restrictive permissions (600)
- [ ] Environment variables used for all secrets
- [ ] `.env.example` has placeholder values only
- [ ] No real credentials in test files
- [ ] Documentation doesn't contain real keys

**Pre-commit Check:**
```bash
# Search for potential secrets
git diff --cached | grep -i "private.*key\|secret\|password\|api.*key"

# Should return nothing or only comments/docs
```

---

## 🚨 If You Accidentally Commit a Secret

### Immediate Actions

1. **Rotate the compromised secret immediately**
   - Generate new keypair
   - Rotate API keys
   - Update environment variables

2. **Remove from Git history** (use with caution)
   ```bash
   # Remove file from entire history
   git filter-branch --force --index-filter \
     "git rm --cached --ignore-unmatch wallet.json" \
     --prune-empty --tag-name-filter cat -- --all
   
   # Force push (WARNING: Rewrites history)
   git push origin --force --all
   ```

3. **Notify team members**
   - Alert anyone who has cloned the repository
   - Ensure they pull the cleaned history

4. **Use BFG Repo-Cleaner** (recommended alternative)
   ```bash
   # Install BFG
   brew install bfg
   
   # Remove sensitive data
   bfg --delete-files wallet.json
   
   # Clean up and push
   git reflog expire --expire=now --all
   git gc --prune=now --aggressive
   git push --force
   ```

---

## 📚 Additional Resources

- [Solana Security Best Practices](https://docs.solana.com/developing/programming-model/security)
- [GitHub Secret Scanning](https://docs.github.com/en/code-security/secret-scanning)
- [OWASP Secrets Management](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html)
- [git-secrets Tool](https://github.com/awslabs/git-secrets) - Prevents committing secrets

---

## 🔐 Production Deployment

### For Production Environments:

1. **Use Hardware Wallets** (Ledger, Trezor) for high-value operations
2. **Implement Multi-Signature** wallets for critical actions
3. **Use Secret Management Services:**
   - AWS Secrets Manager
   - HashiCorp Vault
   - Google Cloud Secret Manager
   - Azure Key Vault

4. **Enable Monitoring:**
   - Alert on unauthorized access attempts
   - Log all wallet operations
   - Monitor for unusual transactions

5. **Regular Security Audits:**
   - Review access logs
   - Rotate credentials quarterly
   - Test disaster recovery procedures

---

## ⚠️ Current Implementation Status

### ✅ Security Features Implemented:
- Environment variable configuration for all secrets
- Keypair loading from secure files
- `.gitignore` protects sensitive files
- No hardcoded private keys in codebase
- Secure wallet initialization with fallback options

### 🚧 Production Hardening Needed:
- Hardware wallet integration
- Multi-signature support
- Secrets management service integration
- Comprehensive audit logging
- Automated secret rotation

**Remember:** Security is an ongoing process, not a one-time setup. Stay vigilant!
