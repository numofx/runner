// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface INumoEnginePool {
    function sellFYToken(address to, uint128 minBaseOut) external returns (uint128);
    function sellBase(address to, uint128 minFYOut) external returns (uint128);
    function buyFYToken(uint128 fyOut, address to) external returns (uint128 baseIn);
    function buyBase(uint128 baseOut, address to) external returns (uint128 fyIn);

    function sellFYTokenPreview(uint128 fyIn) external view returns (uint128 baseOut);
    function buyFYTokenPreview(uint128 fyOut) external view returns (uint128 baseIn);
    function sellBasePreview(uint128 baseIn) external view returns (uint128 fyOut);
    function buyBasePreview(uint128 baseOut) external view returns (uint128 fyIn);

    function getCache() external view returns (uint128 baseReserves, uint128 fyReserves, uint16 feeBps);
    function maturity() external view returns (uint32);
}

interface IERC20 {
    function approve(address spender, uint256 amount) external returns (bool);
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}

/// @title NumoArbRouter
/// @notice Minimal router that performs a cheap→rich cycle in one atomic transaction:
///   1) Acquire FY on cheap pool (buyFYToken or sellBase flow)
///   2) Sell FY into rich pool
/// @dev Caller must pre-approve base tokens to this router if using buyFY path.
contract NumoArbRouter {

    address public immutable baseToken;  // USDT or other base asset
    address public immutable fyToken;    // fyUSDT or other FY token

    error TransferFailed();
    error SlippageExceeded();
    error InsufficientProfit();

    constructor(address _baseToken, address _fyToken) {
        baseToken = _baseToken;
        fyToken = _fyToken;
    }

    /// @notice Execute arbitrage by buying FY on cheap pool and selling on rich pool
    /// @param cheapPool Pool where FY is underpriced (buy FY here)
    /// @param richPool Pool where FY is overpriced (sell FY here)
    /// @param fyOutTarget Amount of FY tokens to acquire from cheap pool
    /// @param maxBaseIn Maximum base tokens willing to spend buying FY
    /// @param minBaseOutRich Minimum base tokens to receive when selling FY
    /// @param receiver Address to receive profits
    /// @return baseSpent Amount of base tokens spent buying FY
    /// @return baseReceived Amount of base tokens received selling FY
    function arbBuyFYThenSellFY(
        address cheapPool,
        address richPool,
        uint128 fyOutTarget,
        uint128 maxBaseIn,
        uint128 minBaseOutRich,
        address receiver
    ) external returns (uint128 baseSpent, uint128 baseReceived) {
        IERC20 base = IERC20(baseToken);
        IERC20 fy = IERC20(fyToken);

        // Pull base tokens from caller to pay for FY purchase
        if (!base.transferFrom(msg.sender, address(this), maxBaseIn)) {
            revert TransferFailed();
        }

        // Approve cheap pool to spend base tokens
        base.approve(cheapPool, type(uint256).max);

        // 1) Buy FY on cheap pool
        baseSpent = INumoEnginePool(cheapPool).buyFYToken(fyOutTarget, address(this));
        if (baseSpent > maxBaseIn) {
            revert SlippageExceeded();
        }

        // 2) Approve rich pool to spend FY tokens
        fy.approve(richPool, type(uint256).max);

        // 3) Sell FY on rich pool
        baseReceived = INumoEnginePool(richPool).sellFYToken(receiver, minBaseOutRich);
        if (baseReceived < minBaseOutRich) {
            revert SlippageExceeded();
        }

        // 4) Ensure we made a profit
        if (baseReceived <= baseSpent) {
            revert InsufficientProfit();
        }

        // 5) Return any leftover base tokens to caller
        uint256 leftover = base.balanceOf(address(this));
        if (leftover > 0) {
            if (!base.transfer(msg.sender, leftover)) {
                revert TransferFailed();
            }
        }

        return (baseSpent, baseReceived);
    }

    /// @notice Execute arbitrage by selling base for FY on cheap pool, then selling FY on rich pool
    /// @dev Alternative flow when sellBase is more gas efficient
    /// @param cheapPool Pool where FY is underpriced (sell base for FY here)
    /// @param richPool Pool where FY is overpriced (sell FY here)
    /// @param baseIn Amount of base tokens to sell for FY
    /// @param minFYOut Minimum FY tokens to receive from cheap pool
    /// @param minBaseOut Minimum base tokens to receive from rich pool
    /// @param receiver Address to receive profits
    /// @return fyAcquired Amount of FY tokens acquired from cheap pool
    /// @return baseReceived Amount of base tokens received from rich pool
    function arbSellBaseThenSellFY(
        address cheapPool,
        address richPool,
        uint128 baseIn,
        uint128 minFYOut,
        uint128 minBaseOut,
        address receiver
    ) external returns (uint128 fyAcquired, uint128 baseReceived) {
        IERC20 base = IERC20(baseToken);
        IERC20 fy = IERC20(fyToken);

        // Pull base tokens from caller
        if (!base.transferFrom(msg.sender, address(this), baseIn)) {
            revert TransferFailed();
        }

        // Approve cheap pool to spend base tokens
        base.approve(cheapPool, type(uint256).max);

        // 1) Sell base for FY on cheap pool
        fyAcquired = INumoEnginePool(cheapPool).sellBase(address(this), minFYOut);
        if (fyAcquired < minFYOut) {
            revert SlippageExceeded();
        }

        // 2) Approve rich pool to spend FY tokens
        fy.approve(richPool, type(uint256).max);

        // 3) Sell FY on rich pool
        baseReceived = INumoEnginePool(richPool).sellFYToken(receiver, minBaseOut);
        if (baseReceived < minBaseOut) {
            revert SlippageExceeded();
        }

        // 4) Ensure we made a profit
        if (baseReceived <= baseIn) {
            revert InsufficientProfit();
        }

        return (fyAcquired, baseReceived);
    }

    /// @notice Emergency function to recover stuck tokens
    /// @param token Token address to recover
    /// @param to Recipient address
    /// @param amount Amount to recover
    function recover(address token, address to, uint256 amount) external {
        // In production, add onlyOwner or similar access control
        IERC20(token).transfer(to, amount);
    }
}
