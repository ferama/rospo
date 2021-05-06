package cmd

import (
	"fmt"
	"path/filepath"

	"github.com/ferama/rospo/pkg/utils"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(keygenCmd)

	keygenCmd.Flags().BoolP("store", "s", false, "optional store the keys to files")
	keygenCmd.Flags().StringP("path", "p", ".", "key pair destination path")
	keygenCmd.Flags().StringP("name", "n", "identity", "output file name")
}

var keygenCmd = &cobra.Command{
	Use:   "keygen",
	Short: "Generates private/public key pairs",
	Long:  `Generates private/public key pairs`,
	Run: func(cmd *cobra.Command, args []string) {
		path, _ := cmd.Flags().GetString("path")
		name, _ := cmd.Flags().GetString("name")
		storeKeys, _ := cmd.Flags().GetBool("store")

		key, err := utils.GeneratePrivateKey()
		if err != nil {
			panic(err)
		}
		publicKey, err := utils.GeneratePublicKey(&key.PublicKey)
		if err != nil {
			panic(err)
		}
		encodedKey := utils.EncodePrivateKeyToPEM(key)
		if storeKeys {
			utils.WriteKeyToFile(encodedKey, filepath.Join(path, name))
			utils.WriteKeyToFile(publicKey, filepath.Join(path, name+".pub"))
		} else {
			fmt.Printf("%s", encodedKey)
			fmt.Printf("%s", publicKey)
		}
	},
}
