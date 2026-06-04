use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, system_instruction};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("4SC596ZDseRDiTYk6ivUxLKBGHVy9mTTUBA74DqFp1hi");

#[program]
pub mod dutch_auction {
    use super::*;

    pub fn initialize_auction(
        ctx: Context<InitializeAuction>,
        starting_price: u64,
        floor_price: u64,
        duration: i64, // in seconds
    ) -> Result<()> {
				// Initialize the auction account and set seller details
        let auction = &mut ctx.accounts.auction;
        auction.seller = ctx.accounts.seller.key();
        auction.starting_price = starting_price;
        auction.floor_price = floor_price;
        auction.duration = duration;
        auction.start_time = Clock::get()?.unix_timestamp;
        auction.token_mint = ctx.accounts.mint.key();

        // Move 1 token from seller ATA into vault escrow
        let cpi_accounts = Transfer {
            from: ctx.accounts.seller_ata.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.seller.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, 1)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeAuction<'info> {
    #[account(init, payer = seller, space = 8 + Auction::INIT_SPACE)]
    pub auction: Account<'info, Auction>,

    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = seller
    )]
    pub seller_ata: Account<'info, TokenAccount>,

    /// CHECK: This is the PDA that will own the vault
    #[account(
        seeds = [b"vault", auction.key().as_ref()],
        bump
    )]
    pub vault_auth: UncheckedAccount<'info>,

    #[account(
        init,
        payer = seller,
        associated_token::mint = mint,
        associated_token::authority = vault_auth
    )]
    pub vault: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct Auction {
    pub seller: Pubkey,
    pub starting_price: u64,
    pub floor_price: u64,
    pub duration: i64,
    pub start_time: i64,
    pub token_mint: Pubkey,
    pub sold: bool,
}
