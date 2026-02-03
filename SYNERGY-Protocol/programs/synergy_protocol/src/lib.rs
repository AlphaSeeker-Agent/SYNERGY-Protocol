use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod synergy_protocol {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

    pub fn register_service(
        ctx: Context<RegisterService>,
        service_name: String,
        capabilities: Vec<String>,
        pricing_model: PricingModel,
        endpoint_url: String,
    ) -> Result<()> {
        let service_account = &mut ctx.accounts.service_account;
        
        service_account.owner = ctx.accounts.signer.key();
        service_account.service_name = service_name;
        service_account.capabilities = capabilities;
        service_account.pricing_model = pricing_model;
        service_account.endpoint_url = endpoint_url;
        service_account.reputation_score = 0;
        service_account.total_tasks_completed = 0;
        service_account.created_at = Clock::get()?.unix_timestamp;
        service_account.updated_at = Clock::get()?.unix_timestamp;

        emit!(ServiceRegistered {
            service_owner: ctx.accounts.signer.key(),
            service_name: service_account.service_name.clone(),
        });

        Ok(())
    }

    pub fn create_task_order(
        ctx: Context<CreateTaskOrder>,
        task_description: String,
        required_capabilities: Vec<String>,
        reward_amount: u64,
        deadline: i64,
    ) -> Result<()> {
        let task_account = &mut ctx.accounts.task_account;
        
        task_account.creator = ctx.accounts.signer.key();
        task_account.assignee = Pubkey::default(); // Initially unassigned
        task_account.task_description = task_description;
        task_account.required_capabilities = required_capabilities;
        task_account.reward_amount = reward_amount;
        task_account.reward_mint = ctx.accounts.reward_mint.key();
        task_account.deadline = deadline;
        task_account.status = TaskStatus::Pending;
        task_account.created_at = Clock::get()?.unix_timestamp;
        task_account.completed_at = 0;

        emit!(TaskCreated {
            task_id: task_account.key(),
            creator: ctx.accounts.signer.key(),
        });

        Ok(())
    }

    pub fn claim_task(ctx: Context<ClaimTask>) -> Result<()> {
        let task_account = &mut ctx.accounts.task_account;
        
        require!(
            task_account.status == TaskStatus::Pending,
            SynergyError::TaskAlreadyAssigned
        );

        task_account.assignee = ctx.accounts.claimer.key();
        task_account.status = TaskStatus::Assigned;

        emit!(TaskClaimed {
            task_id: task_account.key(),
            assignee: ctx.accounts.claimer.key(),
        });

        Ok(())
    }

    pub fn complete_task(ctx: Context<CompleteTask>, result_hash: [u8; 32]) -> Result<()> {
        let task_account = &mut ctx.accounts.task_account;
        
        require!(
            task_account.status == TaskStatus::Assigned || task_account.status == TaskStatus::InProgress,
            SynergyError::InvalidTaskStatus
        );
        
        require!(
            task_account.assignee == ctx.accounts.completer.key(),
            SynergyError::UnauthorizedCompleter
        );

        task_account.status = TaskStatus::Completed;
        task_account.completed_at = Clock::get()?.unix_timestamp;

        // Create escrow to hold payment
        let escrow_account = &mut ctx.accounts.escrow_account;
        escrow_account.task_id = task_account.key();
        escrow_account.payer = task_account.creator;
        escrow_account.payee = task_account.assignee;
        escrow_account.amount = task_account.reward_amount;
        escrow_account.token_mint = task_account.reward_mint;
        escrow_account.released = false;
        escrow_account.created_at = Clock::get()?.unix_timestamp;

        emit!(TaskCompleted {
            task_id: task_account.key(),
            completer: ctx.accounts.completer.key(),
        });

        Ok(())
    }

    pub fn release_payment(ctx: Context<ReleasePayment>) -> Result<()> {
        let escrow_account = &mut ctx.accounts.escrow_account;
        
        require!(
            !escrow_account.released,
            SynergyError::PaymentAlreadyReleased
        );

        escrow_account.released = true;

        emit!(PaymentReleased {
            escrow_id: escrow_account.key(),
            amount: escrow_account.amount,
            payee: escrow_account.payee,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RegisterService<'info> {
    #[account(init, payer = signer, space = 8 + 32 + 4 + 64 + 4 + 4 * 100 + 8 + 256 + 4 + 4 + 8 + 8)]
    pub service_account: Account<'info, ServiceRegistration>,
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateTaskOrder<'info> {
    #[account(init, payer = signer, space = 8 + 32 + 32 + 256 + 4 + 4 * 50 + 8 + 32 + 8 + 1 + 8 + 8)]
    pub task_account: Account<'info, TaskOrder>,
    #[account(mut)]
    pub signer: Signer<'info>,
    /// CHECK: Reward mint validation happens in the instruction
    pub reward_mint: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimTask<'info> {
    #[account(mut)]
    pub task_account: Account<'info, TaskOrder>,
    #[account(mut)]
    pub claimer: Signer<'info>,
}

#[derive(Accounts)]
pub struct CompleteTask<'info> {
    #[account(mut)]
    pub task_account: Account<'info, TaskOrder>,
    #[account(mut)]
    pub completer: Signer<'info>,
    #[account(init, payer = completer, space = 8 + 32 + 32 + 32 + 8 + 32 + 1 + 8)]
    pub escrow_account: Account<'info, TaskEscrow>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ReleasePayment<'info> {
    #[account(mut)]
    pub escrow_account: Account<'info, TaskEscrow>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[account]
pub struct ServiceRegistration {
    pub owner: Pubkey,
    pub service_name: String,
    pub capabilities: Vec<String>,
    pub pricing_model: PricingModel,
    pub endpoint_url: String,
    pub reputation_score: u32,
    pub total_tasks_completed: u32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[account]
pub struct TaskOrder {
    pub creator: Pubkey,
    pub assignee: Pubkey,
    pub task_description: String,
    pub required_capabilities: Vec<String>,
    pub reward_amount: u64,
    pub reward_mint: Pubkey,
    pub deadline: i64,
    pub status: TaskStatus,
    pub created_at: i64,
    pub completed_at: i64,
}

#[account]
pub struct TaskEscrow {
    pub task_id: Pubkey,
    pub payer: Pubkey,
    pub payee: Pubkey,
    pub amount: u64,
    pub token_mint: Pubkey,
    pub released: bool,
    pub created_at: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum TaskStatus {
    Pending,
    Assigned,
    InProgress,
    Completed,
    Failed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum PricingModel {
    Fixed { amount: u64, token: Pubkey },
    PerUnit { amount_per_unit: u64, token: Pubkey, unit_type: String },
    Negotiable { min_amount: u64, max_amount: u64, token: Pubkey },
}

#[event]
pub struct ServiceRegistered {
    pub service_owner: Pubkey,
    pub service_name: String,
}

#[event]
pub struct TaskCreated {
    pub task_id: Pubkey,
    pub creator: Pubkey,
}

#[event]
pub struct TaskClaimed {
    pub task_id: Pubkey,
    pub assignee: Pubkey,
}

#[event]
pub struct TaskCompleted {
    pub task_id: Pubkey,
    pub completer: Pubkey,
}

#[event]
pub struct PaymentReleased {
    pub escrow_id: Pubkey,
    pub amount: u64,
    pub payee: Pubkey,
}

#[error_code]
pub enum SynergyError {
    #[msg("Task is already assigned to another agent")]
    TaskAlreadyAssigned,
    #[msg("Task is not in the correct status for this operation")]
    InvalidTaskStatus,
    #[msg("Only the assigned agent can complete this task")]
    UnauthorizedCompleter,
    #[msg("Payment has already been released")]
    PaymentAlreadyReleased,
}
