import React from 'react'
import { http } from '../lib/Axios'
import { Table, PageHeader, Divider } from 'antd';
import { CreateForm } from '../components/CreateForm';
import { Row, Col, Button } from 'antd';
import {
    DeleteOutlined,
  } from '@ant-design/icons';

export class Tunnels extends React.Component {
    constructor(props) {
        super(props)
        this.state = {
            tuns: [],
            showForm: false
        }
        this.intervalHandler = null
    }

    async componentDidMount() {
        await this.getAll()
        this.intervalHandler = setInterval(this.getAll, 5000)
    }
    componentWillUnmount() {
        clearInterval(this.intervalHandler)
    }

    getAll = async () => {
        let data
        try {
            data = await http.get("tuns/")
        } catch {
            return
        }
        if (data.data === null) {
            this.setState({
                tuns: []
            })
            return
        }
        
        const tuns = []

        for (let t of data.data) {
            t["key"] = parseInt(t.Id)
            tuns.push(t)
        }
        // always sort by key
        tuns.sort( (a, b) => {
            if (a.key < b.key) return -1
            if (a.key > b.key) return 1
            return 0
        })

        this.setState({
            tuns: tuns
        })
    }

    onNewClick = () => {
        const showForm = !this.state.showForm
        this.setState({
            showForm: showForm
        })
    }

    onFormFinish = async (values) => {
        await http.post('tuns/', values)
        this.setState({
            showForm: false
        })
    }

    onDelete = async (id) => {
        await http.delete(`/tuns/${id}`)
        await this.getAll()
    }
    
    render () {
        const columns = [
            {
                title: 'Id',
                dataIndex: 'Id',
                key: '1',
            },
            {
                title: 'Listener',
                dataIndex: 'Listener',
                key: '2',
                render: item => { return item ? `${item.IP}:${item.Port}` : "" }
            },
            {
                title: 'Is Local Listener',
                dataIndex: 'IsListenerLocal',
                key: '3',
                render: item => item ? "true": "false"
            },
            {
                title: 'Endpoint',
                dataIndex: 'Endpoint',
                key: '4',
                render: item => `${item.Host}:${item.Port}`
            },
            {
                title: 'Active Clients',
                dataIndex: 'ClientsCount',
                key: '5',
            },
            {
                title: 'Throughput',
                dataIndex: 'ThroughputString',
                key: '6',
            },
            {
                title: 'Action',
                key: '7',
                render: (_, record) =>  (
                    <React.Fragment>
                        {record.IsStoppable?(
                            <Button onClick={ (e) => this.onDelete(record.Id)} >
                            <DeleteOutlined /> 
                            </Button>
                        ):""}
                    </React.Fragment>
                ),
            }
        ]
        return (
            <React.Fragment>
                <Row>
                    <Col flex="auto">
                        <PageHeader title="Tunnels" />
                    </Col>    
                    <Col flex="100px">
                        <Button type="primary" onClick={this.onNewClick}>New Tunnel</Button>
                    </Col>    
                </Row>
                {this.state.showForm?(
                    <React.Fragment>
                        <CreateForm showForward={true} onFinish={this.onFormFinish}/>
                        <Divider />
                    </React.Fragment>
                ): ""}
                <Table columns={columns} dataSource={this.state.tuns} />
            </React.Fragment>
        )
    }
} 