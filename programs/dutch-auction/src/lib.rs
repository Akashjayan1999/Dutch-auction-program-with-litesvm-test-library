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

    pub fn buy(ctx: Context<Buy>) -> Result<()> {
        // Check if the NFT is already sold
        require!(
            ctx.accounts.auction.sold == false,
            AuctionError::NFTAlreadySold
        );
        let auction = &mut ctx.accounts.auction;
        let now = Clock::get()?.unix_timestamp; // Get the current time from the clock sysvar

        // Validate auction timing
        require!(now >= auction.start_time, AuctionError::AuctionNotStarted);
        require!(
            now < auction.start_time + auction.duration,
            AuctionError::AuctionEnded
        );

        // Calculate current price based on elapsed time (linear decay)
        let elapsed_time = (now - auction.start_time).min(auction.duration) as u64;
        let total_price_drop = auction.starting_price - auction.floor_price;
        let price_dropped_so_far = total_price_drop * elapsed_time / auction.duration as u64;
        let price = auction.starting_price - price_dropped_so_far;

        // Verify funds and transfer payment
        require!(
            ctx.accounts.buyer.lamports() >= price,
            AuctionError::InsufficientFunds
        );
        invoke(
            &system_instruction::transfer(
                &ctx.accounts.buyer.key(),
                &ctx.accounts.seller.key(),
                price,
            ),
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.seller.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        // Transfer NFT to buyer
        let auction_key = ctx.accounts.auction.key();
        let vault_auth_bump = ctx.bumps.vault_auth;
        let bump_slice = &[vault_auth_bump]; // ✅ stored separately so it's &[u8]

        let vault_signer_seeds: &[&[u8]] = &[b"vault", auction_key.as_ref(), bump_slice];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.buyer_ata.to_account_info(),
                    authority: ctx.accounts.vault_auth.to_account_info(),
                },
                &[vault_signer_seeds],
            ),
            1, // transfer 1 token (the auctioned NFT)
        )?;

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


#[derive(Accounts)]
pub struct Buy<'info> {
    #[account(mut, has_one = seller)] // ensure we pass the right auction account
    pub auction: Account<'info, Auction>, // auction account
    /// CHECK: seller account
    #[account(mut)]
    pub seller: AccountInfo<'info>, // seller account
    #[account(mut)]
    pub buyer: Signer<'info>, // buyer account

    #[account(
        mut,
        associated_token::mint = auction.token_mint,
        associated_token::authority = buyer
    )]
    pub buyer_ata: Account<'info, TokenAccount>, // Buyer's ATA

    #[account(
        mut,
        seeds = [b"vault", auction.key().as_ref()],
        bump
    )]
    /// CHECK: PDA authority for the vault
    pub vault_auth: AccountInfo<'info>, // Vault authority PDA

    #[account(
        mut,
        associated_token::mint = auction.token_mint,
        associated_token::authority = vault_auth
    )]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>, // SPL Token program
    pub system_program: Program<'info, System>, // System program
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


#[error_code]
pub enum AuctionError {
    #[msg("Auction hasn't started")]
    AuctionNotStarted,
    #[msg("Buyer has insufficient funds")]
    InsufficientFunds,
    #[msg("Auction has ended")]
    AuctionEnded,
    #[msg("NFT is already sold")]
    NFTAlreadySold,
}

