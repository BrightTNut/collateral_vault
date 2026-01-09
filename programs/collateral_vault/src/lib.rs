use anchor_lang::prelude::*;
use anchor_spl::token::{ self, Transfer };
use anchor_spl::token::{ Token, TokenAccount, Mint };
mod create;
mod ds;
mod deposit;
use create::Vault;

use ds::{ VaultAuthority, CollateralVault, TransactionRecord, TransactionType };
declare_id!("FBN2vp46nz2C3PFcfDLr5uaZPUCi4eiFGJxSEBovQRMV");

#[program]
pub mod collateral_vault {
    use super::*;


    pub fn initialize(ctx: Context<Initialize>, bump: u8) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.owner = ctx.accounts.user.key();
        vault.token_account = ctx.accounts.token_vault.key();
        vault.bump = bump;
        vault.created_at = Clock::get()?.unix_timestamp;

        vault.total_balance = 0;
        vault.locked_balance = 0;
        vault.available_balance = 0;
        vault.total_deposited = 0;
        vault.total_withdrawn = 0;

        Ok(())
    }

    pub fn initialize_authority(ctx: Context<InitializeAuthority>, bump: u8) -> Result<()> {
        let auth = &mut ctx.accounts.vault_authority;
        auth.authorized_programs = Vec::new(); 
        auth.bump = bump;
        Ok(())
    }

   
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        let vault = &mut ctx.accounts.vault;

        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.token_vault.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        vault.total_balance = vault.total_balance.checked_add(amount).ok_or(ErrorCode::Overflow)?;
        vault.available_balance = vault.available_balance
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;
        vault.total_deposited = vault.total_deposited
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;

        emit!(TransactionRecord {
            vault: vault.key(),
            transaction_type: TransactionType::Deposit,
            amount,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        let vault = &mut ctx.accounts.vault;

        require!(vault.available_balance >= amount, ErrorCode::InsufficientFunds);
        require!(ctx.accounts.owner.key() == vault.owner, ErrorCode::Unauthorized);

        vault.total_balance = vault.total_balance.checked_sub(amount).ok_or(ErrorCode::Underflow)?;
        vault.available_balance = vault.available_balance
            .checked_sub(amount)
            .ok_or(ErrorCode::Underflow)?;
        vault.total_withdrawn = vault.total_withdrawn
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;
        let seeds = &[b"vault".as_ref(), vault.owner.as_ref(), &[vault.bump]];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.token_vault.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new_with_signer(cpi_program, cpi_accounts, signer), amount)?;

        emit!(TransactionRecord {
            vault: vault.key(),
            transaction_type: TransactionType::Withdrawal,
            amount,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    
    pub fn authorize_program(ctx: Context<ManageAuthority>, program_id: Pubkey) -> Result<()> {
        let auth = &mut ctx.accounts.vault_authority;
      if !auth.authorized_programs.contains(&program_id) {
            auth.authorized_programs.push(program_id);
        }
        Ok(())
    }

    pub fn lock_collateral(ctx: Context<LockUnlock>, amount: u64) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        let auth = &ctx.accounts.vault_authority;
        let caller = ctx.accounts.caller_program.key();

        require!(auth.authorized_programs.contains(&caller), ErrorCode::UnauthorizedProgram);

        require!(vault.available_balance >= amount, ErrorCode::InsufficientFunds);

        vault.available_balance = vault.available_balance
            .checked_sub(amount)
            .ok_or(ErrorCode::Underflow)?;
        vault.locked_balance = vault.locked_balance.checked_add(amount).ok_or(ErrorCode::Overflow)?;

        emit!(TransactionRecord {
            vault: vault.key(),
            transaction_type: TransactionType::Lock,
            amount,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    pub fn unlock_collateral(ctx: Context<LockUnlock>, amount: u64) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        let auth = &ctx.accounts.vault_authority;
        let caller = ctx.accounts.caller_program.key();

        require!(auth.authorized_programs.contains(&caller), ErrorCode::UnauthorizedProgram);

        require!(vault.locked_balance >= amount, ErrorCode::MathError);

        vault.locked_balance = vault.locked_balance
            .checked_sub(amount)
            .ok_or(ErrorCode::Underflow)?;
        vault.available_balance = vault.available_balance
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;

        emit!(TransactionRecord {
            vault: vault.key(),
            transaction_type: TransactionType::Unlock,
            amount,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub usdt_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = user,
        space = Vault::LEN,
        seeds = [b"vault", user.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, CollateralVault>,
    #[account(
        init,
        payer = user,
        seeds = [b"token_vault", user.key().as_ref()],
        bump,
        token::mint = usdt_mint,
        token::authority = vault
    )]
    pub token_vault: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct InitializeAuthority<'info> {
    #[account(
        init,
        seeds = [b"authority"],
        bump,
        payer = payer,
        space = 8 + 4 + 32 * 5 + 1 
    )]
    pub vault_authority: Account<'info, VaultAuthority>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut, has_one = owner)]
    pub vault: Account<'info, CollateralVault>,
    #[account(mut)]
    pub token_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut, has_one = owner)]
    pub vault: Account<'info, CollateralVault>,
    #[account(mut, address = vault.token_account)]
    pub token_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ManageAuthority<'info> {
    #[account(mut, seeds = [b"authority"], bump = vault_authority.bump)]
    pub vault_authority: Account<'info, VaultAuthority>,
    pub admin: Signer<'info>, 
}

#[derive(Accounts)]
pub struct LockUnlock<'info> {
    #[account(mut)]
    pub vault: Account<'info, CollateralVault>,
    #[account(seeds = [b"authority"], bump = vault_authority.bump)]
    pub vault_authority: Account<'info, VaultAuthority>,
     /// CHECK: The caller is checked in the instruction logic to ensure it is in the authorized_programs list.
      pub caller_program: UncheckedAccount<'info>,
}

// --- Errors ---

#[error_code]
pub enum ErrorCode {
    #[msg("InvalidAmount.")]
    InvalidAmount,
    #[msg("Calculation overflow.")]
    Overflow,
    #[msg("Calculation underflow.")]
    Underflow,
    #[msg("Insufficient available balance.")]
    InsufficientFunds,
    #[msg("Math error.")]
    MathError,
    #[msg("Unauthorized access.")]
    Unauthorized,
    #[msg("Program not authorized to lock/unlock.")]
    UnauthorizedProgram,
}
