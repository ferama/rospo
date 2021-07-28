import React from "react";
import { Form, Input, Button, Checkbox } from 'antd';


export class CreateForm extends React.Component {
    constructor(props) {
        super(props)
        this.state = {}
    }

    render() {
        return (
            <Form
                name="basic"
                labelCol={{ span: 8 }}
                wrapperCol={{ span: 8 }}
                layout="horizontal"
                initialValues={{ forward: false }}
                onFinish={this.props.onFinish}
                // onFinishFailed={this.onFinishFailed}
                >
                <Form.Item
                    label="Local"
                    name="local"
                    rules={[{ required: true, message: 'Please input the local endpoint' }]}
                >
                    <Input />
                </Form.Item>

                <Form.Item
                    label="Remote"
                    name="remote"
                    rules={[{ required: true, message: 'Please input the remote endpoint' }]}
                >
                    <Input />
                </Form.Item>
                {this.props.showForward? (
                    <Form.Item name="forward" valuePropName="checked" wrapperCol={{ offset: 8, span: 16 }}>
                        <Checkbox>Is Local Listener</Checkbox>
                    </Form.Item>
                ):""}

                <Form.Item wrapperCol={{ offset: 8, span: 16 }}>
                    <Button type="primary" htmlType="submit">
                        Submit
                    </Button>
                </Form.Item>
            </Form>
        )
    }
}