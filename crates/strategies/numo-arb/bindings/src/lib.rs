use ethers::contract::abigen;

// Numo Engine Pool interface
abigen!(
    NumoEnginePool,
    r#"[
        function sellFYToken(address to, uint128 minBaseOut) external returns (uint128)
        function sellBase(address to, uint128 minFYOut) external returns (uint128)
        function buyFYToken(uint128 fyOut, address to) external returns (uint128 baseIn)
        function buyBase(uint128 baseOut, address to) external returns (uint128 fyIn)
        function sellFYTokenPreview(uint128 fyIn) external view returns (uint128 baseOut)
        function buyFYTokenPreview(uint128 fyOut) external view returns (uint128 baseIn)
        function sellBasePreview(uint128 baseIn) external view returns (uint128 fyOut)
        function buyBasePreview(uint128 baseOut) external view returns (uint128 fyIn)
        function getCache() external view returns (uint128 baseReserves, uint128 fyReserves, uint16 feeBps)
        function maturity() external view returns (uint32)
    ]"#
);

// Numo Arbitrage Router interface
abigen!(
    NumoArbRouter,
    r#"[
        function arbBuyFYThenSellFY(address cheapPool, address richPool, uint128 fyOutTarget, uint128 maxBaseIn, uint128 minBaseOutRich, address receiver) external returns (uint128 baseSpent, uint128 baseReceived)
        function arbSellBaseThenSellFY(address cheapPool, address richPool, uint128 baseIn, uint128 minFYOut, uint128 minBaseOut, address receiver) external returns (uint128 fyAcquired, uint128 baseReceived)
    ]"#
);

// ERC20 interface for base and FY tokens
abigen!(
    ERC20,
    r#"[
        function approve(address spender, uint256 amount) external returns (bool)
        function transfer(address to, uint256 amount) external returns (bool)
        function transferFrom(address from, address to, uint256 amount) external returns (bool)
        function balanceOf(address account) external view returns (uint256)
        function allowance(address owner, address spender) external view returns (uint256)
    ]"#
);
