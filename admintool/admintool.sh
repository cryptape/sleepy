#!/bin/bash
display_help()
{
	echo 
	echo "usage: $0 -l ip_list -d block_duration -h HZ"
	echo "option:"
	echo "-l ip_list     list all the node's IP and port"
	echo "    default value is '127.0.0.1:4000,127.0.0.1:4001,127.0.0.1:4002,127.0.0.1:4003'"
	echo
	echo "-d block_duration    block generating duration(second)"
	echo "    default value is '6'"
	echo
	echo "-h HZ    times try to generate block per second"
	echo "    default value is '10'"
	echo
	exit 0
}

# parse options
while getopts 'a:p:l:n:m:d:tb:f:' OPT; do
    case $OPT in
        l)
            IP_LIST="$OPTARG";;
        d)
            DURATION="$OPTARG";;
        h)
            HZ="$OPTARG";;
        ?)
            display_help
    esac
done

#set default value
if [ ! -n "$IP_LIST" ]; then
	DEV_MOD=1
    IP_LIST="127.0.0.1:4000,127.0.0.1:4001,127.0.0.1:4002,127.0.0.1:4003"
fi

if [ ! -n "$DURATION" ]; then
    DURATION=6
fi

if [ ! -n "$HZ" ]; then
    HZ=10
fi

#calc size of nodes
TMP=${IP_LIST//[^\:]}
SIZE=${#TMP}

DATA_PATH=`pwd`/release

rm -rf $DATA_PATH

if [ ! -f "$DATA_PATH" ]; then
    mkdir -p $DATA_PATH
fi

for ((ID=0;ID<$SIZE;ID++))
do
	mkdir -p $DATA_PATH/node$ID
	echo "Start generating private Key for Node" $ID "!"
	python create_keys_addr.py $DATA_PATH $ID
	echo "[PrivateKey Path] : " $DATA_PATH/node$ID
	echo "End generating private Key for Node" $ID "!"
done

for ((ID=0;ID<$SIZE;ID++))
do
	echo "Start creating Network Node" $ID "Configuration!"
	python create_config.py $DATA_PATH $ID $SIZE $IP_LIST $DURATION $HZ
	echo "End creating Network Node" $ID "Configuration!"
	echo "########################################################"
done

for ((ID=0;ID<$SIZE;ID++))
do
    echo "Start creating Node " $ID " env!"
	echo "Start copy binary and migrations for Node " $ID "!"
	cp -rf $DATA_PATH/../../target/debug/sleepy $DATA_PATH/node$ID/
done
echo "********************************************************"
echo "WARN: remember then delete all privkey files!!!"