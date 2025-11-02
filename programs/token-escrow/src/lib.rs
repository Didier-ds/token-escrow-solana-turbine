use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, Transfer};

declare_id!("DdCnHPAZi1kNJzZ9tSJvNz4nY11XsuzGZWsp6ASqtHpt");

#[program]
pub mod token_escrow {
    use super::*;

    /// Initialize an escrow
    /// Alice locks her DED tokens and sets the exchange terms
    pub fn initialize_escrow(
        ctx: Context<InitializeEscrow>,
        amount_to_send: u64,      // Amount of DED tokens Alice is offering
        amount_to_receive: u64,   // Amount of SOL Alice wants in return (lamports)
    ) -> Result<()> {
        let escrow_account = &mut ctx.accounts.escrow_account;

        escrow_account.initializer = ctx.accounts.initializer.key();
        escrow_account.initializer_token_account = ctx.accounts.initializer_token_account.key();
        escrow_account.amount_to_send = amount_to_send;
        escrow_account.amount_to_receive = amount_to_receive;
        escrow_account.mint = ctx.accounts.mint.key();
        escrow_account.escrow_bump = ctx.bumps.escrow_account;
        escrow_account.vault_bump = ctx.bumps.vault;
        escrow_account.is_completed = false;

        // Transfer tokens from Alice to escrow vault
        let cpi_accounts = Transfer {
            from: ctx.accounts.initializer_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.initializer.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        token::transfer(cpi_ctx, amount_to_send)?;

        msg!("Escrow initialized! {} DED tokens locked", amount_to_send);
        msg!("Seller wants {} lamports (SOL)", amount_to_receive);

        Ok(())
    }

    /// Complete the escrow
    /// Bob pays SOL and receives Alice's DED tokens
    pub fn exchange(ctx: Context<Exchange>) -> Result<()> {
        let escrow_account = &ctx.accounts.escrow_account;

        // Verify escrow is not already completed
        require!(!escrow_account.is_completed, EscrowError::AlreadyCompleted);

        // Transfer SOL from taker (Bob) to initializer (Alice)
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.taker.key(),
            &ctx.accounts.initializer.key(),
            escrow_account.amount_to_receive,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.taker.to_account_info(),
                ctx.accounts.initializer.to_account_info(),
            ],
        )?;

        // Transfer DED tokens from vault to taker (Bob)
        let seeds = &[
            b"vault",
            escrow_account.initializer.as_ref(),
            &[escrow_account.vault_bump],
        ];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.taker_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

        token::transfer(cpi_ctx, escrow_account.amount_to_send)?;

        // Mark escrow as completed
        let escrow_account = &mut ctx.accounts.escrow_account;
        escrow_account.is_completed = true;

        msg!("Escrow completed! Tokens and SOL exchanged");

        Ok(())
    }

    /// Cancel the escrow and return tokens to Alice
    pub fn cancel(ctx: Context<Cancel>) -> Result<()> {
        let escrow_account = &ctx.accounts.escrow_account;

        // Verify escrow is not already completed
        require!(!escrow_account.is_completed, EscrowError::AlreadyCompleted);

        // Return tokens to initializer
        let seeds = &[
            b"vault",
            escrow_account.initializer.as_ref(),
            &[escrow_account.vault_bump],
        ];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.initializer_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

        token::transfer(cpi_ctx, escrow_account.amount_to_send)?;

        msg!("Escrow cancelled! Tokens returned");

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeEscrow<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,

    pub mint: Account<'info, anchor_spl::token::Mint>,

    #[account(
        mut,
        constraint = initializer_token_account.owner == initializer.key(),
        constraint = initializer_token_account.mint == mint.key()
    )]
    pub initializer_token_account: Account<'info, anchor_spl::token::TokenAccount>,

    #[account(
        init,
        payer = initializer,
        space = 8 + EscrowAccount::INIT_SPACE,
        seeds = [b"escrow", initializer.key().as_ref()],
        bump
    )]
    pub escrow_account: Account<'info, EscrowAccount>,

    #[account(
        init,
        payer = initializer,
        seeds = [b"vault", initializer.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = vault,
    )]
    pub vault: Account<'info, anchor_spl::token::TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Exchange<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,

    /// CHECK: This is the initializer who will receive SOL
    #[account(mut)]
    pub initializer: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = taker_token_account.owner == taker.key(),
        constraint = taker_token_account.mint == escrow_account.mint
    )]
    pub taker_token_account: Account<'info, anchor_spl::token::TokenAccount>,

    #[account(
        mut,
        seeds = [b"vault", escrow_account.initializer.as_ref()],
        bump = escrow_account.vault_bump,
    )]
    pub vault: Account<'info, anchor_spl::token::TokenAccount>,

    #[account(
        mut,
        seeds = [b"escrow", escrow_account.initializer.as_ref()],
        bump = escrow_account.escrow_bump,
        has_one = initializer,
        has_one = mint,
    )]
    pub escrow_account: Account<'info, EscrowAccount>,

    pub mint: Account<'info, anchor_spl::token::Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Cancel<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,

    #[account(
        mut,
        constraint = initializer_token_account.owner == initializer.key(),
    )]
    pub initializer_token_account: Account<'info, anchor_spl::token::TokenAccount>,

    #[account(
        mut,
        seeds = [b"vault", escrow_account.initializer.as_ref()],
        bump = escrow_account.vault_bump,
    )]
    pub vault: Account<'info, anchor_spl::token::TokenAccount>,

    #[account(
        mut,
        seeds = [b"escrow", initializer.key().as_ref()],
        bump = escrow_account.escrow_bump,
        has_one = initializer,
        close = initializer
    )]
    pub escrow_account: Account<'info, EscrowAccount>,

    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(InitSpace)]
pub struct EscrowAccount {
    pub initializer: Pubkey,
    pub initializer_token_account: Pubkey,
    pub amount_to_send: u64,
    pub amount_to_receive: u64,
    pub mint: Pubkey,
    pub escrow_bump: u8,
    pub vault_bump: u8,
    pub is_completed: bool,
}

#[error_code]
pub enum EscrowError {
    #[msg("Escrow has already been completed")]
    AlreadyCompleted,
}