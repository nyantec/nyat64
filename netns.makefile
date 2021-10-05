
create-netns:
	sudo ip netns add $(NET_ROUTER)
	sudo ip netns exec $(NET_ROUTER) ip link set dev lo up
	#sudo ip netns add $(NET_SRC)
	#sudo ip netns exec $(NET_SRC) ip link set dev lo up
	sudo ip netns add $(NET_DST)
	sudo ip netns exec $(NET_DST) ip link set dev lo up

delete-netns:
	-sudo ip netns pids $(NET_ROUTER) | xargs sudo kill
	#-sudo ip netns pids $(NET_SRC) | xargs sudo kill
	-sudo ip netns pids $(NET_DST) | xargs sudo kill
	#-sudo ip netns exec $(NET_ROUTER) ip link delete router-src
	-sudo ip netns exec $(NET_ROUTER) ip link delete router-dst
	#-sudo ip netns exec $(NET_SRC) ip link delete src-router
	-sudo ip netns exec $(NET_DST) ip link delete dst-router
	-sudo ip netns delete $(NET_ROUTER)
	#-sudo ip netns delete $(NET_SRC)
	-sudo ip netns delete $(NET_DST)

create-links: create-netns create-link-dst-router #create-link-src-router
	@echo "Giving the kernel enough time to register addresses"
	@sleep 2s

#create-link-src-router:
#	sudo ip link add name router-src type veth peer name src-router
#	sudo ip link set router-src netns $(NET_ROUTER)
#	sudo ip link set src-router netns $(NET_SRC)
#	sudo ip netns exec $(NET_ROUTER) ip li set dev router-src up
#	sudo ip netns exec $(NET_ROUTER) ip address add 172.16.21.2/24 dev router-src
#	sudo ip netns exec $(NET_ROUTER) ip address add fd57:328e:14dc:e231::2/64 dev router-src
#	sudo ip netns exec $(NET_SRC) ip li set dev src-router up
#	sudo ip netns exec $(NET_SRC) ip address add 172.16.21.1/24 dev src-router
#	sudo ip netns exec $(NET_SRC) ip address add fd57:328e:14dc:e231::1/64 dev src-router
#	sudo ip netns exec $(NET_SRC) ip route add 172.16.20.0/24 via 172.16.21.2 dev src-router
#	sudo ip netns exec $(NET_SRC) ip route add fd00:3da::/96 via fd57:328e:14dc:e231::2 dev src-router*/

create-link-dst-router:
	sudo ip link add name router-dst type veth peer name dst-router
	sudo ip link set router-dst netns $(NET_ROUTER)
	sudo ip link set dst-router netns $(NET_DST)
	sudo ip netns exec $(NET_ROUTER) ip li set dev router-dst up
	sudo ip netns exec $(NET_ROUTER) ip address add 172.16.20.2/24 dev router-dst
	sudo ip netns exec $(NET_DST) ip li set dev dst-router up
	sudo ip netns exec $(NET_DST) ip address add 172.16.20.1/24 dev dst-router


setupt-netns-common: create-links
