package cmd

import (
	"log"

	"github.com/ferama/rospo/pkg/pipe"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(pipeCmd)

	pipeCmd.Flags().StringP("local", "l", "", "the local pipe endpoint")
	pipeCmd.Flags().StringP("remote", "r", "", "the remote pipe endpoint")
}

var pipeCmd = &cobra.Command{
	Use:   "pipe",
	Short: "Starts a pipe ",
	Args:  cobra.MinimumNArgs(1),
	Long:  "Starts a pipe",
	Run: func(cmd *cobra.Command, args []string) {
		local, _ := cmd.Flags().GetString("local")
		remote, _ := cmd.Flags().GetString("remote")
		if local == "" || remote == "" {
			log.Fatalf("local and remote enpoint should not be empty")
		}
		conf := &pipe.PipeConf{
			Local:  local,
			Remote: remote,
		}

		p := pipe.NewPipe(conf, false)
		p.Start()
	},
}
