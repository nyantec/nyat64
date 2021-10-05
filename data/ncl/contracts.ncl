{
  InterfaceName | doc "A contract for an interface name" =
    fun label value =>
      if builtins.isStr value &&
        (strings.length value) <= 16 &&
        (strings.length value) >= 1
      then
        value
      else
        contracts.blame label,

  InterfaceConfig | doc "A contract for an interface config" = {
    name | #(InterfaceName)
    // address (optional)
    // mask (optional)
    // mtu (optional)
  },

  Interface | doc "A contract for an interface" =
    fun label value =>
      if builtins.isStr value then
        value | #(InterfaceName)
      else if builtins.isRecord value then
        value | #(InterfaceConfig)
      else
        contracts.blame label,


  Ipv4Address | doc "A contract for a ipv4 address" =
    fun l v =>
      if builtins.isStr v
      then
        v
      else
        contracts.blame l,

  Ipv6Address | doc "A contract for a ipv6 address" =
    fun l v =>
      if builtins.isStr v
      then
        v
      else
        contracts.blame l,

  Mapping | doc "A ipv6 to ipv4 mapping entry" = {
    ipv6_remote | #Ipv6Address,
		//| doc "ipv6 destination address to handle,
    ipv6_local  | #(Ipv6Address),
		//| doc "ipv6 source address to handle,
    ipv4_local  | #(Ipv4Address),
                //| doc "ipv4 source address to map,
    ipv4_remote | #(Ipv4Address),
                //| doc "ipv4 destination address to map,
    // gateway (optional)

  },

  Nyat64 = {
    interfaces = {
      ipv6 | #(Interface)
           | doc "ipv6 src interface"
           | default = "mytun",
      ipv4 | #(Interface)
           | doc "ipv4 dst interface"
    },
    mappings | List #Mapping
  },

}
