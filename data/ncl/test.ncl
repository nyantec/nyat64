// Nat 64 test config

let Nyat64 = (import "contracts.ncl").Nyat64 in

{
  interfaces = {
    ipv4 = "router-dst",
    ipv6 = {
        name = "mytun",
        address = "fd00:3da::1",
    }
  },
  mappings = [
    {
      ipv6_remote = "fd00:3da::1",
      ipv6_local = "fd00:3da::2",
      ipv4_remote = "172.16.20.1",
      ipv4_local = "172.16.20.2",
    }//,
    //{
    //  ipv6_remote = "fd00:3da::21",
    //  ipv6_local = "fd00:3da::22",
    //  ipv4_remote = "192.168.23.23",
    //  ipv4_local = "10.18.255.254",
    //  ipv4_gateway = "10.18.5.25"
    //}
  ]
} |#Nyat64
